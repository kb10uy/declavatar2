use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use nalgebra::{Quaternion, UnitQuaternion, Vector2, Vector3, Vector4};

use super::wire::{Decode, DecodeError, Encode, EncodeError, Reader, Writer};
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

impl Encode for SourceLocation {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.chunk.encode(writer)?;
        self.line.encode(writer)
    }
}

impl Decode for SourceLocation {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            chunk: reader.decode()?,
            line: reader.decode()?,
        })
    }
}

impl Encode for AssetLocator {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            AssetLocator::Guid(guid) => {
                writer.u8(0);
                guid.encode(writer)
            }
            AssetLocator::Path(path) => {
                writer.u8(1);
                path.encode(writer)
            }
            AssetLocator::Named { asset_type, name } => {
                writer.u8(2);
                asset_type.encode(writer)?;
                name.encode(writer)
            }
        }
    }
}

impl Decode for AssetLocator {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(AssetLocator::Guid(reader.decode()?)),
            1 => Ok(AssetLocator::Path(reader.decode()?)),
            2 => Ok(AssetLocator::Named {
                asset_type: reader.decode()?,
                name: reader.decode()?,
            }),
            value => Err(unknown("AssetLocator", value)),
        }
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

impl Encode for ExpressionParameter {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.name.encode(writer)?;
        self.type_default.encode(writer)?;
        self.saved.encode(writer)?;
        self.synced.encode(writer)
    }
}

impl Decode for ExpressionParameter {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            name: reader.decode()?,
            type_default: reader.decode()?,
            saved: reader.decode()?,
            synced: reader.decode()?,
        })
    }
}

impl Encode for ExpressionParameterTypeDefault {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            ExpressionParameterTypeDefault::Bool(default) => {
                writer.u8(0);
                default.encode(writer)
            }
            ExpressionParameterTypeDefault::Int { width, default } => {
                writer.u8(1);
                width.encode(writer)?;
                default.encode(writer)
            }
            ExpressionParameterTypeDefault::Float { width, default } => {
                writer.u8(2);
                width.encode(writer)?;
                default.encode(writer)
            }
        }
    }
}

impl Decode for ExpressionParameterTypeDefault {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(ExpressionParameterTypeDefault::Bool(reader.decode()?)),
            1 => Ok(ExpressionParameterTypeDefault::Int {
                width: reader.decode()?,
                default: reader.decode()?,
            }),
            2 => Ok(ExpressionParameterTypeDefault::Float {
                width: reader.decode()?,
                default: reader.decode()?,
            }),
            value => Err(unknown("ExpressionParameterTypeDefault", value)),
        }
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

impl Encode for PlayableLayer {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u8(match self {
            PlayableLayer::Base => 0,
            PlayableLayer::Additive => 1,
            PlayableLayer::Gesture => 2,
            PlayableLayer::Action => 3,
            PlayableLayer::Fx => 4,
            PlayableLayer::Sitting => 5,
            PlayableLayer::TPose => 6,
            PlayableLayer::IkPose => 7,
        });
        Ok(())
    }
}

impl Decode for PlayableLayer {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => PlayableLayer::Base,
            1 => PlayableLayer::Additive,
            2 => PlayableLayer::Gesture,
            3 => PlayableLayer::Action,
            4 => PlayableLayer::Fx,
            5 => PlayableLayer::Sitting,
            6 => PlayableLayer::TPose,
            7 => PlayableLayer::IkPose,
            value => return Err(unknown("PlayableLayer", value)),
        })
    }
}

impl Encode for MergeMode {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u8(match self {
            MergeMode::Append => 0,
            MergeMode::Replace => 1,
        });
        Ok(())
    }
}

impl Decode for MergeMode {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => MergeMode::Append,
            1 => MergeMode::Replace,
            value => return Err(unknown("MergeMode", value)),
        })
    }
}

impl Encode for PathMode {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u8(match self {
            PathMode::Absolute => 0,
            PathMode::Relative => 1,
        });
        Ok(())
    }
}

impl Decode for PathMode {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => PathMode::Absolute,
            1 => PathMode::Relative,
            value => return Err(unknown("PathMode", value)),
        })
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

impl Encode for AnimatorParameter {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.name.encode(writer)?;
        self.type_default.encode(writer)
    }
}

impl Decode for AnimatorParameter {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            name: reader.decode()?,
            type_default: reader.decode()?,
        })
    }
}

impl Encode for AnimatorParameterTypeDefault {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            AnimatorParameterTypeDefault::Bool(default) => {
                writer.u8(0);
                default.encode(writer)
            }
            AnimatorParameterTypeDefault::Int(default) => {
                writer.u8(1);
                default.encode(writer)
            }
            AnimatorParameterTypeDefault::Float(default) => {
                writer.u8(2);
                default.encode(writer)
            }
        }
    }
}

impl Decode for AnimatorParameterTypeDefault {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(AnimatorParameterTypeDefault::Bool(reader.decode()?)),
            1 => Ok(AnimatorParameterTypeDefault::Int(reader.decode()?)),
            2 => Ok(AnimatorParameterTypeDefault::Float(reader.decode()?)),
            value => Err(unknown("AnimatorParameterTypeDefault", value)),
        }
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

impl Encode for AnimatorTransition {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.from.encode(writer)?;
        self.to.encode(writer)?;
        self.duration.encode(writer)?;
        self.conditions.encode(writer)
    }
}

impl Decode for AnimatorTransition {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            from: reader.decode()?,
            to: reader.decode()?,
            duration: reader.decode()?,
            conditions: reader.decode()?,
        })
    }
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

impl Encode for AnimatorCondition {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            AnimatorCondition::If(parameter) => {
                writer.u8(0);
                parameter.encode(writer)
            }
            AnimatorCondition::IfNot(parameter) => {
                writer.u8(1);
                parameter.encode(writer)
            }
            AnimatorCondition::Equals(parameter, value) => {
                writer.u8(2);
                parameter.encode(writer)?;
                value.encode(writer)
            }
            AnimatorCondition::NotEqual(parameter, value) => {
                writer.u8(3);
                parameter.encode(writer)?;
                value.encode(writer)
            }
            AnimatorCondition::Greater(parameter, value) => {
                writer.u8(4);
                parameter.encode(writer)?;
                value.encode(writer)
            }
            AnimatorCondition::Less(parameter, value) => {
                writer.u8(5);
                parameter.encode(writer)?;
                value.encode(writer)
            }
        }
    }
}

impl Decode for AnimatorCondition {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(AnimatorCondition::If(reader.decode()?)),
            1 => Ok(AnimatorCondition::IfNot(reader.decode()?)),
            2 => Ok(AnimatorCondition::Equals(reader.decode()?, reader.decode()?)),
            3 => Ok(AnimatorCondition::NotEqual(reader.decode()?, reader.decode()?)),
            4 => Ok(AnimatorCondition::Greater(reader.decode()?, reader.decode()?)),
            5 => Ok(AnimatorCondition::Less(reader.decode()?, reader.decode()?)),
            value => Err(unknown("AnimatorCondition", value)),
        }
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
        match reader.u8()? {
            0 => Ok(Motion::Clip(Clip::Inline(reader.decode()?))),
            1 => Ok(Motion::Clip(Clip::External(reader.decode()?))),
            2 => Ok(Motion::BlendTree(BlendTree::Parametric(decode_parametric_tree(reader)?))),
            3 => Ok(Motion::BlendTree(BlendTree::Direct(decode_direct_tree(reader)?))),
            value => Err(unknown("Motion", value)),
        }
    }
}

impl Encode for Clip {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            Clip::Inline(animation) => {
                writer.u8(0);
                animation.encode(writer)
            }
            Clip::External(asset) => {
                writer.u8(1);
                asset.encode(writer)
            }
        }
    }
}

impl Decode for Clip {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(Clip::Inline(reader.decode()?)),
            1 => Ok(Clip::External(reader.decode()?)),
            value => Err(unknown("Clip", value)),
        }
    }
}

impl Encode for BlendTree {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            BlendTree::Parametric(tree) => {
                writer.u8(2);
                tree.tree_type.encode(writer)?;
                tree.x.encode(writer)?;
                tree.y.encode(writer)?;
                tree.fields.encode(writer)
            }
            BlendTree::Direct(tree) => {
                writer.u8(3);
                tree.fields.encode(writer)
            }
        }
    }
}

impl Decode for BlendTree {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            2 => Ok(BlendTree::Parametric(decode_parametric_tree(reader)?)),
            3 => Ok(BlendTree::Direct(decode_direct_tree(reader)?)),
            value => Err(unknown("BlendTree", value)),
        }
    }
}

fn decode_parametric_tree(reader: &mut Reader<'_>) -> Result<ParametricBlendTree, DecodeError> {
    Ok(ParametricBlendTree {
        tree_type: reader.decode()?,
        x: reader.decode()?,
        y: reader.decode()?,
        fields: reader.decode()?,
    })
}

fn decode_direct_tree(reader: &mut Reader<'_>) -> Result<DirectBlendTree, DecodeError> {
    Ok(DirectBlendTree { fields: reader.decode()? })
}

impl Encode for BlendTreeType {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u8(match self {
            BlendTreeType::Linear => 0,
            BlendTreeType::Simple2d => 1,
            BlendTreeType::Freeform2d => 2,
            BlendTreeType::Cartesian2d => 3,
        });
        Ok(())
    }
}

impl Decode for BlendTreeType {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => BlendTreeType::Linear,
            1 => BlendTreeType::Simple2d,
            2 => BlendTreeType::Freeform2d,
            3 => BlendTreeType::Cartesian2d,
            value => return Err(unknown("BlendTreeType", value)),
        })
    }
}

impl Encode for ParametricField {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.f64(self.position[0]);
        writer.f64(self.position[1]);
        self.speed.encode(writer)?;
        self.motion.encode(writer)
    }
}

impl Decode for ParametricField {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            position: [reader.f64()?, reader.f64()?],
            speed: reader.decode()?,
            motion: reader.decode()?,
        })
    }
}

impl Encode for DirectField {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.weight_by.encode(writer)?;
        self.speed.encode(writer)?;
        self.motion.encode(writer)
    }
}

impl Decode for DirectField {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            weight_by: reader.decode()?,
            speed: reader.decode()?,
            motion: reader.decode()?,
        })
    }
}

fn encode_value_set<E: MaybeZeroableEntry + Encode>(set: &ValueSet<E>, writer: &mut Writer) -> Result<(), EncodeError> {
    writer.length("list length", set.len())?;
    for (_, entry) in set.entries() {
        entry.encode(writer)?;
    }
    Ok(())
}

fn decode_value_set<E: MaybeZeroableEntry + Decode>(reader: &mut Reader<'_>) -> Result<ValueSet<E>, DecodeError>
where
    E::Key: Debug,
{
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

impl Encode for InlineAnimation<Compiled> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            InlineAnimation::Fixed(entries) => {
                writer.u8(0);
                encode_value_set(entries, writer)
            }
            InlineAnimation::Keyed(animation) => {
                writer.u8(1);
                animation.attributes.encode(writer)?;
                encode_value_set(&animation.curves, writer)
            }
        }
    }
}

impl Decode for InlineAnimation<Compiled> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(InlineAnimation::Fixed(decode_value_set(reader)?)),
            1 => Ok(InlineAnimation::Keyed(KeyedAnimation {
                attributes: reader.decode()?,
                curves: decode_value_set(reader)?,
            })),
            value => Err(unknown("InlineAnimation", value)),
        }
    }
}

impl Encode for FixedAnimationEntry<Compiled> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.key.encode(writer)?;
        self.value.encode(writer)
    }
}

impl Decode for FixedAnimationEntry<Compiled> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            key: reader.decode()?,
            value: reader.decode()?,
        })
    }
}

impl Encode for KeyedAnimationEntry<Compiled> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.key.encode(writer)?;
        self.curve.encode(writer)
    }
}

impl Decode for KeyedAnimationEntry<Compiled> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            key: reader.decode()?,
            curve: reader.decode()?,
        })
    }
}

impl Encode for ClipAttributes {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.length.encode(writer)?;
        self.loop_time.encode(writer)?;
        self.loop_blend.encode(writer)?;
        self.cycle_offset.encode(writer)
    }
}

impl Decode for ClipAttributes {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            length: reader.decode()?,
            loop_time: reader.decode()?,
            loop_blend: reader.decode()?,
            cycle_offset: reader.decode()?,
        })
    }
}

impl<R: WireObjectRef> Encode for Curve<R> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.first.encode(writer)?;
        self.rest.encode(writer)
    }
}

impl<R: WireObjectRef> Decode for Curve<R> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            first: reader.decode()?,
            rest: reader.decode()?,
        })
    }
}

impl<R: WireObjectRef> Encode for (Interpolation, Keyframe<R>) {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.0.encode(writer)?;
        self.1.encode(writer)
    }
}

impl<R: WireObjectRef> Decode for (Interpolation, Keyframe<R>) {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok((reader.decode()?, reader.decode()?))
    }
}

impl Encode for Interpolation {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            Interpolation::Constant => writer.u8(0),
            Interpolation::Linear => writer.u8(1),
            Interpolation::Bezier { x1, y1, x2, y2 } => {
                writer.u8(2);
                writer.f64(*x1);
                writer.f64(*y1);
                writer.f64(*x2);
                writer.f64(*y2);
            }
        }
        Ok(())
    }
}

impl Decode for Interpolation {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(Interpolation::Constant),
            1 => Ok(Interpolation::Linear),
            2 => Ok(Interpolation::Bezier {
                x1: reader.f64()?,
                y1: reader.f64()?,
                x2: reader.f64()?,
                y2: reader.f64()?,
            }),
            value => Err(unknown("Interpolation", value)),
        }
    }
}

impl<R: WireObjectRef> Encode for Keyframe<R> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.time.encode(writer)?;
        self.value.encode(writer)
    }
}

impl<R: WireObjectRef> Decode for Keyframe<R> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            time: reader.decode()?,
            value: reader.decode()?,
        })
    }
}

impl Encode for AnimatedTarget<Compiled> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            AnimatedTarget::AnimatorSelf(target) => {
                writer.u8(0);
                target.property.encode(writer)
            }
            AnimatedTarget::GameObject(target) => {
                writer.u8(1);
                target.path.encode(writer)?;
                target.property.encode(writer)
            }
            AnimatedTarget::Renderer(target) => {
                writer.u8(2);
                target.path.encode(writer)?;
                target.renderer_type.encode(writer)?;
                target.property.encode(writer)
            }
            AnimatedTarget::Component(target) => {
                writer.u8(3);
                target.path.encode(writer)?;
                target.component_type.encode(writer)?;
                target.property.encode(writer)?;
                target.value_type.encode(writer)
            }
        }
    }
}

impl Decode for AnimatedTarget<Compiled> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget { property: reader.decode()? })),
            1 => Ok(AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: reader.decode()?,
                property: reader.decode()?,
            })),
            2 => Ok(AnimatedTarget::Renderer(AnimatedRendererTarget {
                path: reader.decode()?,
                renderer_type: reader.decode()?,
                property: reader.decode()?,
            })),
            3 => Ok(AnimatedTarget::Component(AnimatedComponentTarget {
                path: reader.decode()?,
                component_type: reader.decode()?,
                property: reader.decode()?,
                value_type: reader.decode()?,
            })),
            value => Err(unknown("AnimatedTarget", value)),
        }
    }
}

impl Encode for AnimatedAnimatorProperty<Compiled> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            AnimatedAnimatorProperty::ParameterFloatValue { name } => {
                writer.u8(0);
                name.encode(writer)
            }
        }
    }
}

impl Decode for AnimatedAnimatorProperty<Compiled> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(AnimatedAnimatorProperty::ParameterFloatValue { name: reader.decode()? }),
            value => Err(unknown("AnimatedAnimatorProperty", value)),
        }
    }
}

impl Encode for AnimatedGameObjectProperty {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u8(match self {
            AnimatedGameObjectProperty::Active => 0,
            AnimatedGameObjectProperty::TransformPosition => 1,
            AnimatedGameObjectProperty::TransformRotationQuaternion => 2,
            AnimatedGameObjectProperty::TransformRotationEuler => 3,
            AnimatedGameObjectProperty::TransformScale => 4,
        });
        Ok(())
    }
}

impl Decode for AnimatedGameObjectProperty {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => AnimatedGameObjectProperty::Active,
            1 => AnimatedGameObjectProperty::TransformPosition,
            2 => AnimatedGameObjectProperty::TransformRotationQuaternion,
            3 => AnimatedGameObjectProperty::TransformRotationEuler,
            4 => AnimatedGameObjectProperty::TransformScale,
            value => return Err(unknown("AnimatedGameObjectProperty", value)),
        })
    }
}

impl Encode for AnimatedRendererProperty {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            AnimatedRendererProperty::Enabled => writer.u8(0),
            AnimatedRendererProperty::BlendShape { name } => {
                writer.u8(1);
                name.encode(writer)?;
            }
            AnimatedRendererProperty::Material { slot } => {
                writer.u8(2);
                writer.u32(*slot);
            }
            AnimatedRendererProperty::MaterialProperty { name } => {
                writer.u8(3);
                name.encode(writer)?;
            }
            AnimatedRendererProperty::Serialized { name } => {
                writer.u8(4);
                name.encode(writer)?;
            }
        }
        Ok(())
    }
}

impl Decode for AnimatedRendererProperty {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(AnimatedRendererProperty::Enabled),
            1 => Ok(AnimatedRendererProperty::BlendShape { name: reader.decode()? }),
            2 => Ok(AnimatedRendererProperty::Material { slot: reader.u32()? }),
            3 => Ok(AnimatedRendererProperty::MaterialProperty { name: reader.decode()? }),
            4 => Ok(AnimatedRendererProperty::Serialized { name: reader.decode()? }),
            value => Err(unknown("AnimatedRendererProperty", value)),
        }
    }
}

impl Encode for AnimatedComponentProperty {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            AnimatedComponentProperty::Enabled => writer.u8(0),
            AnimatedComponentProperty::Serialized { name } => {
                writer.u8(1);
                name.encode(writer)?;
            }
        }
        Ok(())
    }
}

impl Decode for AnimatedComponentProperty {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(AnimatedComponentProperty::Enabled),
            1 => Ok(AnimatedComponentProperty::Serialized { name: reader.decode()? }),
            value => Err(unknown("AnimatedComponentProperty", value)),
        }
    }
}

impl Encode for AnimatedValueType {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u8(match self {
            AnimatedValueType::Float => 0,
            AnimatedValueType::Int => 1,
            AnimatedValueType::Bool => 2,
            AnimatedValueType::Vector2 => 3,
            AnimatedValueType::Vector3 => 4,
            AnimatedValueType::Vector4 => 5,
            AnimatedValueType::Quaternion => 6,
            AnimatedValueType::Color => 7,
            AnimatedValueType::ObjectReference => 8,
        });
        Ok(())
    }
}

impl Decode for AnimatedValueType {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => AnimatedValueType::Float,
            1 => AnimatedValueType::Int,
            2 => AnimatedValueType::Bool,
            3 => AnimatedValueType::Vector2,
            4 => AnimatedValueType::Vector3,
            5 => AnimatedValueType::Vector4,
            6 => AnimatedValueType::Quaternion,
            7 => AnimatedValueType::Color,
            8 => AnimatedValueType::ObjectReference,
            value => return Err(unknown("AnimatedValueType", value)),
        })
    }
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

impl Encode for Behavior {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            Behavior::ParameterDrive(drive) => {
                writer.u8(0);
                drive.target.encode(writer)
            }
            Behavior::TrackingControl(control) => {
                writer.u8(1);
                control.encode(writer)
            }
            Behavior::Generic(behavior) => {
                writer.u8(2);
                behavior.type_name.encode(writer)?;
                behavior.fields.encode(writer)
            }
        }
    }
}

impl Decode for Behavior {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(Behavior::ParameterDrive(ParameterDrive { target: reader.decode()? })),
            1 => Ok(Behavior::TrackingControl(reader.decode()?)),
            2 => Ok(Behavior::Generic(GenericStateBehavior {
                type_name: reader.decode()?,
                fields: reader.decode()?,
            })),
            value => Err(unknown("Behavior", value)),
        }
    }
}

impl Encode for ParameterDriveTarget<Compiled> {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            ParameterDriveTarget::Set { parameter, value } => {
                writer.u8(0);
                parameter.encode(writer)?;
                value.encode(writer)
            }
            ParameterDriveTarget::Add { parameter, value } => {
                writer.u8(1);
                parameter.encode(writer)?;
                value.encode(writer)
            }
            ParameterDriveTarget::RandomInt { parameter, range } => {
                writer.u8(2);
                parameter.encode(writer)?;
                writer.i64(range[0]);
                writer.i64(range[1]);
                Ok(())
            }
            ParameterDriveTarget::RandomBool { parameter, chance } => {
                writer.u8(3);
                parameter.encode(writer)?;
                writer.f64(*chance);
                Ok(())
            }
            ParameterDriveTarget::RandomFloat { parameter, range } => {
                writer.u8(4);
                parameter.encode(writer)?;
                writer.f64(range[0]);
                writer.f64(range[1]);
                Ok(())
            }
            ParameterDriveTarget::Copy { from, to } => {
                writer.u8(5);
                from.encode(writer)?;
                to.encode(writer)
            }
            ParameterDriveTarget::RangedCopy {
                from,
                from_range,
                to,
                to_range,
            } => {
                writer.u8(6);
                from.encode(writer)?;
                writer.f64(from_range[0]);
                writer.f64(from_range[1]);
                to.encode(writer)?;
                writer.f64(to_range[0]);
                writer.f64(to_range[1]);
                Ok(())
            }
        }
    }
}

impl Decode for ParameterDriveTarget<Compiled> {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(ParameterDriveTarget::Set {
                parameter: reader.decode()?,
                value: reader.decode()?,
            }),
            1 => Ok(ParameterDriveTarget::Add {
                parameter: reader.decode()?,
                value: reader.decode()?,
            }),
            2 => Ok(ParameterDriveTarget::RandomInt {
                parameter: reader.decode()?,
                range: [reader.i64()?, reader.i64()?],
            }),
            3 => Ok(ParameterDriveTarget::RandomBool {
                parameter: reader.decode()?,
                chance: reader.f64()?,
            }),
            4 => Ok(ParameterDriveTarget::RandomFloat {
                parameter: reader.decode()?,
                range: [reader.f64()?, reader.f64()?],
            }),
            5 => Ok(ParameterDriveTarget::Copy {
                from: reader.decode()?,
                to: reader.decode()?,
            }),
            6 => Ok(ParameterDriveTarget::RangedCopy {
                from: reader.decode()?,
                from_range: [reader.f64()?, reader.f64()?],
                to: reader.decode()?,
                to_range: [reader.f64()?, reader.f64()?],
            }),
            value => Err(unknown("ParameterDriveTarget", value)),
        }
    }
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

impl Encode for TrackingControlMode {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        writer.u8(match self {
            TrackingControlMode::NoChange => 0,
            TrackingControlMode::Tracking => 1,
            TrackingControlMode::Animation => 2,
        });
        Ok(())
    }
}

impl Decode for TrackingControlMode {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match reader.u8()? {
            0 => TrackingControlMode::NoChange,
            1 => TrackingControlMode::Tracking,
            2 => TrackingControlMode::Animation,
            value => return Err(unknown("TrackingControlMode", value)),
        })
    }
}

impl Encode for GenericValue {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            GenericValue::Bool(value) => {
                writer.u8(0);
                writer.bool(*value);
                Ok(())
            }
            GenericValue::Int(value) => {
                writer.u8(1);
                writer.i64(*value);
                Ok(())
            }
            GenericValue::Float(value) => {
                writer.u8(2);
                writer.f64(*value);
                Ok(())
            }
            GenericValue::String(value) => {
                writer.u8(3);
                value.encode(writer)
            }
            GenericValue::List(values) => {
                writer.u8(4);
                values.encode(writer)
            }
            GenericValue::Map(values) => {
                writer.u8(5);
                values.encode(writer)
            }
        }
    }
}

impl Decode for GenericValue {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(GenericValue::Bool(reader.bool()?)),
            1 => Ok(GenericValue::Int(reader.i64()?)),
            2 => Ok(GenericValue::Float(reader.f64()?)),
            3 => Ok(GenericValue::String(reader.decode()?)),
            4 => Ok(GenericValue::List(reader.decode()?)),
            5 => Ok(GenericValue::Map(reader.decode()?)),
            value => Err(unknown("GenericValue", value)),
        }
    }
}

impl Encode for MenuItem {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        match self {
            MenuItem::SubMenu { name, items } => {
                writer.u8(0);
                name.encode(writer)?;
                items.encode(writer)
            }
            MenuItem::Toggle { name, parameter, value } => {
                writer.u8(1);
                name.encode(writer)?;
                parameter.encode(writer)?;
                value.encode(writer)
            }
            MenuItem::Button { name, parameter, value } => {
                writer.u8(2);
                name.encode(writer)?;
                parameter.encode(writer)?;
                value.encode(writer)
            }
            MenuItem::Radial { name, axis } => {
                writer.u8(3);
                name.encode(writer)?;
                axis.encode(writer)
            }
            MenuItem::TwoAxis { name, horizontal, vertical } => {
                writer.u8(4);
                name.encode(writer)?;
                horizontal.encode(writer)?;
                vertical.encode(writer)
            }
            MenuItem::FourAxis { name, up, down, left, right } => {
                writer.u8(5);
                name.encode(writer)?;
                up.encode(writer)?;
                down.encode(writer)?;
                left.encode(writer)?;
                right.encode(writer)
            }
        }
    }
}

impl Decode for MenuItem {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match reader.u8()? {
            0 => Ok(MenuItem::SubMenu {
                name: reader.decode()?,
                items: reader.decode()?,
            }),
            1 => Ok(MenuItem::Toggle {
                name: reader.decode()?,
                parameter: reader.decode()?,
                value: reader.decode()?,
            }),
            2 => Ok(MenuItem::Button {
                name: reader.decode()?,
                parameter: reader.decode()?,
                value: reader.decode()?,
            }),
            3 => Ok(MenuItem::Radial {
                name: reader.decode()?,
                axis: reader.decode()?,
            }),
            4 => Ok(MenuItem::TwoAxis {
                name: reader.decode()?,
                horizontal: reader.decode()?,
                vertical: reader.decode()?,
            }),
            5 => Ok(MenuItem::FourAxis {
                name: reader.decode()?,
                up: reader.decode()?,
                down: reader.decode()?,
                left: reader.decode()?,
                right: reader.decode()?,
            }),
            value => Err(unknown("MenuItem", value)),
        }
    }
}

impl Encode for MenuAxis {
    fn encode(&self, writer: &mut Writer) -> Result<(), EncodeError> {
        self.parameter.encode(writer)?;
        self.positive.encode(writer)?;
        self.negative.encode(writer)
    }
}

impl Decode for MenuAxis {
    fn decode(reader: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            parameter: reader.decode()?,
            positive: reader.decode()?,
            negative: reader.decode()?,
        })
    }
}
