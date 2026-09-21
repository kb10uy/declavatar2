use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use nalgebra::{Quaternion, UnitQuaternion, Vector2, Vector3, Vector4};

use super::{
    macros::{wire_enum, wire_struct},
    wire::{Decode, DecodeError, Encode, EncodeError, Reader, Writer},
};
use crate::{
    avatar::{
        AnimatorCondition, AnimatorController, AnimatorLayer, AnimatorState, AnimatorTransition, Avatar, Behavior, BlendTree, Clip, DirectBlendTree,
        DirectField, MenuAxis, MenuItem, Motion, ParametricBlendTree, ParametricField, PlayableController, Playback, TransitionSource, TransitionTarget,
    },
    core::{
        external::{Extern, ExternEntry, ExternKind, ExternTable},
        phase::Compiled,
        resolution::{Resolved, SourceLocation},
        value_set::{MaybeZeroableEntry, ValueSet},
    },
    unity::{
        animation::{ClipAttributes, Curve, FixedAnimationEntry, InlineAnimation, Interpolation, KeyedAnimation, KeyedAnimationEntry, Keyframe},
        animator::{
            AnimatedAnimatorProperty, AnimatedAnimatorTarget, AnimatedComponentProperty, AnimatedComponentTarget, AnimatedGameObjectProperty,
            AnimatedGameObjectTarget, AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget, AnimatorParameter, AnimatorParameterTypeDefault,
            BlendTreeType, MergeMode, PathMode,
        },
        external::{Asset, AssetLocator, ComponentType, Externals, ObjectPath},
        state::{GenericStateBehavior, GenericValue},
        value::{AnimatedValue, AnimatedValueType},
    },
    vrchat::{
        expr_parameter::{ExpressionParameter, ExpressionParameterTypeDefault, ExpressionParameterWidth},
        playable_layer::PlayableLayer,
        state_behaviour::{ParameterDrive, ParameterDriveTarget, TrackingControl, TrackingControlMode, TrackingControlTarget},
    },
};

fn unknown(type_name: &'static str, value: u8) -> DecodeError {
    DecodeError::InvalidDiscriminator { type_name, value }
}

pub(crate) trait ExternSlot: ExternKind {
    const SLOT: usize;
    const NAME: &'static str;
}

impl ExternSlot for ObjectPath {
    const SLOT: usize = 0;
    const NAME: &'static str = "object path";
}

impl ExternSlot for ComponentType {
    const SLOT: usize = 1;
    const NAME: &'static str = "component type";
}

impl ExternSlot for Asset {
    const SLOT: usize = 2;
    const NAME: &'static str = "asset";
}

impl<K: ExternKind> Encode for Extern<K> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u32(self.index());
        Ok(())
    }
}

impl<K: ExternSlot> Decode for Extern<K> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let index = reader.u32()?;
        let len = reader.context().extern_lens[K::SLOT];
        if u64::from(index) >= len as u64 {
            return Err(DecodeError::ExternOutOfRange { kind: K::NAME, index, len });
        }
        Ok(Extern::from_index(index))
    }
}

impl<K: ExternKind> Encode for ExternEntry<K>
where
    K::Value: Encode,
{
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.value.encode(writer)?;
        self.referenced_at.encode(writer)
    }
}

impl<K: ExternKind> Decode for ExternEntry<K>
where
    K::Value: Decode,
{
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            value: reader.decode()?,
            referenced_at: reader.decode()?,
        })
    }
}

impl<K: ExternKind> Encode for ExternTable<K>
where
    K::Value: Encode,
{
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.entries().encode(writer)
    }
}

impl<K: ExternSlot> Decode for ExternTable<K>
where
    K::Value: Decode,
{
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let entries: Vec<ExternEntry<K>> = reader.decode()?;
        ExternTable::from_entries(entries).map_err(|value| DecodeError::Duplicate {
            what: K::NAME,
            key: format!("{value:?}"),
        })
    }
}

wire_struct! {
    SourceLocation { chunk, line }
}

wire_enum! {
    AssetLocator {
        0 Guid(guid),
        1 Path(path),
        2 Named { asset_type, name },
    }
}

impl Encode for Externals {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.object_paths.encode(writer)?;
        self.component_types.encode(writer)?;
        self.assets.encode(writer)?;
        self.needs_relative_root.encode(writer)
    }
}

impl Decode for Externals {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let externals = Self {
            object_paths: reader.decode()?,
            component_types: reader.decode()?,
            assets: reader.decode()?,
            needs_relative_root: reader.decode()?,
        };
        reader.context_mut().extern_lens = [externals.object_paths.len(), externals.component_types.len(), externals.assets.len()];
        Ok(externals)
    }
}

impl<C> Encode for Resolved<String, C> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.value.encode(writer)
    }
}

impl Decode for Resolved<String, AnimatedValueType> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let name = reader.string()?;
        match reader.context().parameters.get(&name) {
            Some(value_type) => Ok(Resolved::new(name, *value_type)),
            None => Err(DecodeError::UnknownParameter(name)),
        }
    }
}

struct StateIndex(usize);

impl Encode for StateIndex {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        let index = u32::try_from(self.0).map_err(|_| EncodeError::Overflow {
            what: "state index",
            value: self.0,
        })?;
        writer.u32(index);
        Ok(())
    }
}

impl Decode for StateIndex {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let index = reader.u32()?;
        let len = reader.context().states.unwrap_or(0);
        check_state_index(index, len).map(StateIndex)
    }
}

fn check_state_index(index: u32, len: usize) -> Result<usize, DecodeError> {
    match usize::try_from(index) {
        Ok(index) if index < len => Ok(index),
        _ => Err(DecodeError::StateOutOfRange { index, len }),
    }
}

fn add_parameter(scope: &mut BTreeMap<String, AnimatedValueType>, name: &str, value_type: AnimatedValueType) -> Result<(), DecodeError> {
    match scope.get(name) {
        Some(first) if *first != value_type => Err(DecodeError::ConflictingParameter {
            name: name.to_owned(),
            first: *first,
            second: value_type,
        }),
        Some(_) => Ok(()),
        None => {
            scope.insert(name.to_owned(), value_type);
            Ok(())
        }
    }
}

fn controller_scope(parameters: &[AnimatorParameter]) -> Result<BTreeMap<String, AnimatedValueType>, DecodeError> {
    let mut scope = BTreeMap::new();
    for parameter in parameters {
        if scope
            .insert(parameter.name.clone(), parameter.type_default.value_type().animated_value_type())
            .is_some()
        {
            return Err(DecodeError::Duplicate {
                what: "animator parameter",
                key: format!("`{}`", parameter.name),
            });
        }
    }
    Ok(scope)
}

impl Encode for Avatar {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.externals.encode(writer)?;
        self.expression_parameters.encode(writer)?;
        self.controllers.encode(writer)?;
        self.menu.encode(writer)
    }
}

impl Decode for Avatar {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let externals: Externals = reader.decode()?;
        let expression_parameters: Vec<ExpressionParameter> = reader.decode()?;
        let controllers: Vec<PlayableController> = reader.decode()?;

        let mut scope = BTreeMap::new();
        for parameter in &expression_parameters {
            add_parameter(&mut scope, &parameter.name, parameter.type_default.animated_value_type())?;
        }
        for parameter in controllers.iter().flat_map(|c| &c.controller.parameters) {
            add_parameter(&mut scope, &parameter.name, parameter.type_default.value_type().animated_value_type())?;
        }
        reader.context_mut().parameters = scope;
        let menu = reader.decode()?;

        Ok(Self {
            expression_parameters,
            controllers,
            menu,
            externals,
        })
    }
}

wire_struct! {
    ExpressionParameter { name, type_default, saved, synced }
}

wire_enum! {
    ExpressionParameterTypeDefault {
        0 Bool(default),
        1 Int { width, default },
        2 Float { width, default },
    }
}

impl Encode for ExpressionParameterWidth {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            ExpressionParameterWidth::Unspecified => writer.u8(0),
            ExpressionParameterWidth::Specified(0) => return Err(EncodeError::Unrepresentable("a specified width of zero bits")),
            ExpressionParameterWidth::Specified(width) => writer.u8(*width),
        }
        Ok(())
    }
}

impl Decode for ExpressionParameterWidth {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => ExpressionParameterWidth::Unspecified,
            width => ExpressionParameterWidth::Specified(width),
        })
    }
}

wire_enum! {
    PlayableLayer {
        0 Base,
        1 Additive,
        2 Gesture,
        3 Action,
        4 Fx,
        5 Sitting,
        6 TPose,
        7 IkPose,
    }

    MergeMode {
        0 Append,
        1 Replace,
    }

    PathMode {
        0 Absolute,
        1 Relative,
    }
}

impl Encode for PlayableController {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.playable.encode(writer)?;
        self.mode.encode(writer)?;
        self.priority.encode(writer)?;
        self.path_mode.encode(writer)?;
        self.mask.encode(writer)?;
        self.controller.parameters.encode(writer)?;
        self.controller.layers.encode(writer)
    }
}

impl Decode for PlayableController {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let playable = reader.decode()?;
        let mode = reader.decode()?;
        let priority = reader.decode()?;
        let path_mode = reader.decode()?;
        let mask = reader.decode()?;
        let parameters: Vec<AnimatorParameter> = reader.decode()?;
        reader.context_mut().parameters = controller_scope(&parameters)?;
        let layers = reader.decode()?;
        Ok(Self {
            playable,
            mode,
            priority,
            path_mode,
            mask,
            controller: AnimatorController { parameters, layers },
        })
    }
}

wire_struct! {
    AnimatorParameter { name, type_default }
}

wire_enum! {
    AnimatorParameterTypeDefault {
        0 Bool(default),
        1 Int(default),
        2 Float(default),
    }
}

impl Encode for AnimatorLayer {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.name.encode(writer)?;
        self.default_state.map(StateIndex).encode(writer)?;
        self.states.encode(writer)?;
        self.transitions.encode(writer)
    }
}

impl Decode for AnimatorLayer {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let name = reader.decode()?;
        let default_state: Option<u32> = reader.decode()?;
        let states: Vec<AnimatorState> = reader.decode()?;
        let default_state = default_state.map(|index| check_state_index(index, states.len())).transpose()?;
        reader.context_mut().states = Some(states.len());
        let transitions = reader.decode();
        reader.context_mut().states = None;
        Ok(Self {
            name,
            default_state,
            states,
            transitions: transitions?,
        })
    }
}

impl Encode for AnimatorState {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.name.encode(writer)?;
        self.motion.encode(writer)?;
        self.playback.speed.encode(writer)?;
        self.playback.speed_by.encode(writer)?;
        self.playback.time_by.encode(writer)?;
        self.write_defaults.encode(writer)?;
        self.behaviors.encode(writer)
    }
}

impl Decode for AnimatorState {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            name: reader.decode()?,
            motion: reader.decode()?,
            playback: Playback {
                speed: reader.decode()?,
                speed_by: reader.decode()?,
                time_by: reader.decode()?,
            },
            write_defaults: reader.decode()?,
            behaviors: reader.decode()?,
        })
    }
}

wire_struct! {
    AnimatorTransition { from, to, duration, conditions }
}

impl Encode for TransitionSource {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            TransitionSource::Entry => writer.u8(0),
            TransitionSource::State(index) => {
                writer.u8(1);
                StateIndex(*index).encode(writer)?;
            }
        }
        Ok(())
    }
}

impl Decode for TransitionSource {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(TransitionSource::Entry),
            1 => Ok(TransitionSource::State(StateIndex::decode(reader)?.0)),
            value => Err(unknown("TransitionSource", value)),
        }
    }
}

impl Encode for TransitionTarget {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            TransitionTarget::State(index) => {
                writer.u8(0);
                StateIndex(*index).encode(writer)?;
            }
            TransitionTarget::Exit => writer.u8(1),
        }
        Ok(())
    }
}

impl Decode for TransitionTarget {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(TransitionTarget::State(StateIndex::decode(reader)?.0)),
            1 => Ok(TransitionTarget::Exit),
            value => Err(unknown("TransitionTarget", value)),
        }
    }
}

wire_enum! {
    AnimatorCondition {
        0 If(parameter),
        1 IfNot(parameter),
        2 Equals(parameter, value),
        3 NotEqual(parameter, value),
        4 Greater(parameter, value),
        5 Less(parameter, value),
    }
}

impl Encode for Motion {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            Motion::Clip(clip) => clip.encode(writer),
            Motion::BlendTree(tree) => tree.encode(writer),
        }
    }
}

impl Decode for Motion {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.peek_u8()? {
            0 | 1 => Ok(Motion::Clip(reader.decode()?)),
            2 | 3 => Ok(Motion::BlendTree(reader.decode()?)),
            value => Err(unknown("Motion", value)),
        }
    }
}

wire_enum! {
    Clip {
        0 Inline(animation),
        1 External(asset),
    }

    BlendTree {
        2 Parametric(tree),
        3 Direct(tree),
    }

    BlendTreeType {
        0 Linear,
        1 Simple2d,
        2 Freeform2d,
        3 Cartesian2d,
    }
}

wire_struct! {
    ParametricBlendTree { tree_type, x, y, fields }
    ParametricField { position, speed, motion }
    DirectBlendTree { fields }
    DirectField { weight_by, speed, motion }
}

impl<E: MaybeZeroableEntry + Encode> Encode for ValueSet<E> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.length("list length", self.len())?;
        for (_, entry) in self.entries() {
            entry.encode(writer)?;
        }
        Ok(())
    }
}

impl<E: MaybeZeroableEntry + Decode> Decode for ValueSet<E>
where
    E::Key: Debug,
{
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let count = reader.length("list length")?;
        let mut set = ValueSet::new();
        for _ in 0..count {
            let entry = E::decode(reader)?;
            let key = entry.key();
            if set.insert(entry).is_some() {
                return Err(DecodeError::Duplicate {
                    what: "animation target",
                    key: format!("{key:?}"),
                });
            }
        }
        Ok(set)
    }
}

wire_enum! {
    InlineAnimation<Compiled> {
        0 Fixed(entries),
        1 Keyed(animation),
    }

    Interpolation {
        0 Constant,
        1 Linear,
        2 Bezier { x1, y1, x2, y2 },
    }
}

wire_struct! {
    KeyedAnimation<Compiled> { attributes, curves }
    FixedAnimationEntry<Compiled> { key, value }
    KeyedAnimationEntry<Compiled> { key, curve }
    ClipAttributes { length, loop_time, loop_blend, cycle_offset }
    Curve<Extern<Asset>> { first, rest }
    Keyframe<Extern<Asset>> { time, value }
}

impl Encode for (Interpolation, Keyframe<Extern<Asset>>) {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.0.encode(writer)?;
        self.1.encode(writer)
    }
}

impl Decode for (Interpolation, Keyframe<Extern<Asset>>) {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok((reader.decode()?, reader.decode()?))
    }
}

wire_enum! {
    AnimatedTarget<Compiled> {
        0 AnimatorSelf(target),
        1 GameObject(target),
        2 Renderer(target),
        3 Component(target),
    }

    AnimatedAnimatorProperty<Compiled> {
        0 ParameterFloatValue { name },
    }

    AnimatedGameObjectProperty {
        0 Active,
        1 TransformPosition,
        2 TransformRotationQuaternion,
        3 TransformRotationEuler,
        4 TransformScale,
    }

    AnimatedRendererProperty {
        0 Enabled,
        1 BlendShape { name },
        2 Material { slot },
        3 MaterialProperty { name },
        4 Serialized { name },
    }

    AnimatedComponentProperty {
        0 Enabled,
        1 Serialized { name },
    }

    AnimatedValueType {
        0 Float,
        1 Int,
        2 Bool,
        3 Vector2,
        4 Vector3,
        5 Vector4,
        6 Quaternion,
        7 Color,
        8 ObjectReference,
    }
}

wire_struct! {
    AnimatedAnimatorTarget<Compiled> { property }
    AnimatedGameObjectTarget<Compiled> { path, property }
    AnimatedRendererTarget<Compiled> { path, renderer_type, property }
    AnimatedComponentTarget<Compiled> { path, component_type, property, value_type }
}

/// How the object reference of an `AnimatedValue<R>` is carried. `()` has no representation, so it never appears.
pub(crate) trait WireObjectRef: Sized {
    fn encode_reference(&self, writer: &mut Writer) -> Result<(), EncodeError>;
    fn decode_reference(reader: &mut Reader<'_>) -> Result<Self, DecodeError>;
}

impl WireObjectRef for Extern<Asset> {
    fn encode_reference(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.encode(writer)
    }

    fn decode_reference(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        reader.decode()
    }
}

impl WireObjectRef for () {
    fn encode_reference(&self, _: &mut Writer) -> Result<(), EncodeError> {
        Err(EncodeError::Unrepresentable("an object reference without an asset"))
    }

    fn decode_reference(_: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Err(unknown("AnimatedValue<()>", 8))
    }
}

impl<R: WireObjectRef> Encode for AnimatedValue<R> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            AnimatedValue::Float(value) => {
                writer.u8(0);
                writer.f64(*value);
            }
            AnimatedValue::Int(value) => {
                writer.u8(1);
                writer.i64(*value);
            }
            AnimatedValue::Bool(value) => {
                writer.u8(2);
                writer.bool(*value);
            }
            AnimatedValue::Vector2(value) => {
                writer.u8(3);
                writer.f64(value.x);
                writer.f64(value.y);
            }
            AnimatedValue::Vector3(value) => {
                writer.u8(4);
                writer.f64(value.x);
                writer.f64(value.y);
                writer.f64(value.z);
            }
            AnimatedValue::Vector4(value) => {
                writer.u8(5);
                writer.f64(value.x);
                writer.f64(value.y);
                writer.f64(value.z);
                writer.f64(value.w);
            }
            AnimatedValue::Quaternion(value) => {
                writer.u8(6);
                writer.f64(value.i);
                writer.f64(value.j);
                writer.f64(value.k);
                writer.f64(value.w);
            }
            AnimatedValue::Color(value) => {
                writer.u8(7);
                writer.f64(value.x);
                writer.f64(value.y);
                writer.f64(value.z);
                writer.f64(value.w);
            }
            AnimatedValue::ObjectReference(reference) => {
                writer.u8(8);
                reference.encode_reference(writer)?;
            }
        }
        Ok(())
    }
}

impl<R: WireObjectRef> Decode for AnimatedValue<R> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => AnimatedValue::Float(reader.f64()?),
            1 => AnimatedValue::Int(reader.i64()?),
            2 => AnimatedValue::Bool(reader.bool()?),
            3 => AnimatedValue::Vector2(Vector2::new(reader.f64()?, reader.f64()?)),
            4 => AnimatedValue::Vector3(Vector3::new(reader.f64()?, reader.f64()?, reader.f64()?)),
            5 => AnimatedValue::Vector4(Vector4::new(reader.f64()?, reader.f64()?, reader.f64()?, reader.f64()?)),
            6 => {
                let (x, y, z, w) = (reader.f64()?, reader.f64()?, reader.f64()?, reader.f64()?);
                AnimatedValue::Quaternion(UnitQuaternion::new_unchecked(Quaternion::new(w, x, y, z)))
            }
            7 => AnimatedValue::Color(Vector4::new(reader.f64()?, reader.f64()?, reader.f64()?, reader.f64()?)),
            8 => AnimatedValue::ObjectReference(R::decode_reference(reader)?),
            value => return Err(unknown("AnimatedValue", value)),
        })
    }
}

wire_enum! {
    Behavior {
        0 ParameterDrive(drive),
        1 TrackingControl(control),
        2 Generic(behavior),
    }

    ParameterDriveTarget<Compiled> {
        0 Set { parameter, value },
        1 Add { parameter, value },
        2 RandomInt { parameter, range },
        3 RandomBool { parameter, chance },
        4 RandomFloat { parameter, range },
        5 Copy { from, to },
        6 RangedCopy { from, from_range, to, to_range },
    }

    TrackingControlMode {
        0 NoChange,
        1 Tracking,
        2 Animation,
    }

    GenericValue {
        0 Bool(value),
        1 Int(value),
        2 Float(value),
        3 String(value),
        4 List(values),
        5 Map(values),
    }
}

wire_struct! {
    ParameterDrive<Compiled> { target }
    GenericStateBehavior { type_name, fields }
}

/// The wire order of tracking control targets, one byte each.
pub(crate) const TRACKING_TARGETS: [TrackingControlTarget; 10] = [
    TrackingControlTarget::Head,
    TrackingControlTarget::LeftHand,
    TrackingControlTarget::RightHand,
    TrackingControlTarget::Hip,
    TrackingControlTarget::LeftFoot,
    TrackingControlTarget::RightFoot,
    TrackingControlTarget::LeftFingers,
    TrackingControlTarget::RightFingers,
    TrackingControlTarget::Eyes,
    TrackingControlTarget::Mouth,
];

impl Encode for TrackingControl {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        for target in TRACKING_TARGETS {
            self.values.get(&target).copied().unwrap_or(TrackingControlMode::NoChange).encode(writer)?;
        }
        Ok(())
    }
}

impl Decode for TrackingControl {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut values = HashMap::new();
        for target in TRACKING_TARGETS {
            let mode = TrackingControlMode::decode(reader)?;
            if mode != TrackingControlMode::NoChange {
                values.insert(target, mode);
            }
        }
        Ok(Self { values })
    }
}

wire_enum! {
    MenuItem {
        0 SubMenu { name, items },
        1 Toggle { name, parameter, value },
        2 Button { name, parameter, value },
        3 Radial { name, axis },
        4 TwoAxis { name, horizontal, vertical },
        5 FourAxis { name, up, down, left, right },
    }
}

wire_struct! {
    MenuAxis { parameter, positive, negative }
}
