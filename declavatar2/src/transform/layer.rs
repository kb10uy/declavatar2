use std::collections::BTreeSet;

use crate::{
    avatar::controller::{
        AnimatorCondition, AnimatorLayer, AnimatorState, AnimatorTransition, BlendTree, Clip, DirectBlendTree, DirectField, Motion, ParameterRef,
        ParametricBlendTree, ParametricField, Playback, TransitionSource, TransitionTarget,
    },
    core::{phase::Declared, resolution::Unresolved, value_set::MaybeZeroableEntry},
    decl::{
        behavior::{Animation, Content},
        layer::{BlendLayer, GroupLayer, Layer, PuppetKeyframe, PuppetLayer, SwitchContent, SwitchLayer},
    },
    transform::{
        animation::{describe, describe_all},
        context::{Context, driver},
        error::{TransformError, TransformErrorKind},
        raw,
    },
    unity::{
        animation::{FixedAnimationEntry, InlineAnimation},
        animator::{AnimatedTarget, BlendTreeType},
        external::AssetLocator,
        value::{AnimatedValue, AnimatedValueType},
    },
};

pub(crate) fn compile(context: &mut Context, layer: &Layer) -> Result<AnimatorLayer, TransformError> {
    match layer {
        Layer::Group(group) => self::group(context, group),
        Layer::Switch(switch) => self::switch(context, switch),
        Layer::Puppet(puppet) => self::puppet(context, puppet),
        Layer::Blend(blend) => self::blend(context, blend),
        Layer::Raw(raw) => raw::compile(context, raw),
    }
    .map_err(|error| error.or_at(layer.at()))
}

/// A state that plays the written targets as one fixed clip.
pub(crate) fn state(context: &mut Context, name: impl Into<String>, content: &Content, write_defaults: bool) -> Result<AnimatorState, TransformError> {
    Ok(AnimatorState {
        name: name.into(),
        motion: Some(Motion::Clip(Clip::Inline(InlineAnimation::Fixed(context.animation(&content.animation)?)))),
        playback: Playback::default(),
        write_defaults,
        behaviors: context.behaviors(&content.behaviors)?,
    })
}

fn transition(from: TransitionSource, to: TransitionTarget, conditions: Vec<AnimatorCondition>) -> AnimatorTransition {
    AnimatorTransition {
        from,
        to,
        duration: 0.0,
        conditions,
    }
}

fn group(context: &mut Context, group: &GroupLayer) -> Result<AnimatorLayer, TransformError> {
    let parameter = context
        .parameters
        .resolve_typed(&driver(group.driven_by.as_ref(), &group.name, group.at.as_ref()), AnimatedValueType::Int)?;

    let mut default = group.default.clone().unwrap_or_default();
    default
        .animation
        .union_fill_as_zero(group.options.iter().map(|option| &option.content.animation))
        .map_err(|targets| {
            TransformErrorKind::OptionsNeedDefault {
                layer: group.name.clone(),
                targets: describe_all(&targets),
            }
            .at(group.at.clone())
        })?;

    let mut states = vec![state(context, "Default", &default, false)?];
    for option in &group.options {
        let mut content = option.content.clone();
        content
            .animation
            .union_from_defaults(&default.animation)
            .expect("every key of an option is in the default after zero filling");
        states.push(state(context, &option.name, &content, false).map_err(|error| error.or_at(option.at.as_ref()))?);
    }

    let symmetric = group.symmetric.unwrap_or(true);
    let mut transitions = Vec::new();
    for index in 1..states.len() {
        let value = index as i64;
        let equals = || vec![AnimatorCondition::Equals(parameter.clone(), value)];
        let differs = || vec![AnimatorCondition::NotEqual(parameter.clone(), value)];
        if symmetric {
            transitions.push(transition(TransitionSource::Entry, TransitionTarget::State(index), equals()));
        } else {
            transitions.push(transition(TransitionSource::State(0), TransitionTarget::State(index), equals()));
            transitions.push(transition(TransitionSource::State(index), TransitionTarget::State(0), differs()));
        }
    }
    if symmetric {
        for index in 1..states.len() {
            let value = index as i64;
            transitions.push(transition(
                TransitionSource::State(index),
                TransitionTarget::Exit,
                vec![AnimatorCondition::NotEqual(parameter.clone(), value)],
            ));
        }
        for index in 1..states.len() {
            let value = index as i64;
            transitions.push(transition(
                TransitionSource::State(0),
                TransitionTarget::Exit,
                vec![AnimatorCondition::Equals(parameter.clone(), value)],
            ));
        }
    }

    Ok(AnimatorLayer {
        name: group.name.clone(),
        default_state: Some(0),
        states,
        transitions,
    })
}

fn switch(context: &mut Context, switch: &SwitchLayer) -> Result<AnimatorLayer, TransformError> {
    let parameter = context
        .parameters
        .resolve_typed(&driver(switch.driven_by.as_ref(), &switch.name, switch.at.as_ref()), AnimatedValueType::Bool)?;

    let (off, on) = match &switch.content {
        SwitchContent::Toggle(content) => {
            let mut off = Content::new();
            for (_, entry) in content.animation.entries() {
                let zeroed = entry.zeroed().ok_or_else(|| {
                    TransformErrorKind::ToggleNeedsBothSides {
                        layer: switch.name.clone(),
                        target: describe(&entry.key),
                    }
                    .at(switch.at.clone())
                })?;
                off.animation.insert(zeroed);
            }
            (off, content.clone())
        }
        SwitchContent::Sides { off, on } => (off.clone(), on.clone()),
    };

    Ok(AnimatorLayer {
        name: switch.name.clone(),
        default_state: Some(0),
        states: vec![state(context, "Disabled", &off, false)?, state(context, "Enabled", &on, false)?],
        transitions: vec![
            transition(
                TransitionSource::State(0),
                TransitionTarget::State(1),
                vec![AnimatorCondition::If(parameter.clone())],
            ),
            transition(
                TransitionSource::State(1),
                TransitionTarget::State(0),
                vec![AnimatorCondition::IfNot(parameter)],
            ),
        ],
    })
}

fn puppet(context: &mut Context, puppet: &PuppetLayer) -> Result<AnimatorLayer, TransformError> {
    let (_, tree, _) = puppet_tree(context, puppet)?;
    Ok(AnimatorLayer {
        name: puppet.name.clone(),
        default_state: Some(0),
        states: vec![AnimatorState {
            name: puppet.name.clone(),
            motion: Some(Motion::BlendTree(BlendTree::Parametric(tree))),
            playback: Playback::default(),
            write_defaults: false,
            behaviors: vec![],
        }],
        transitions: vec![],
    })
}

/// The 1D blend tree of a puppet layer, together with every target it animates.
fn puppet_tree(context: &mut Context, puppet: &PuppetLayer) -> Result<(ParameterRef, ParametricBlendTree, BTreeSet<AnimatedTarget<Declared>>), TransformError> {
    let parameter = context
        .parameters
        .resolve_typed(&driver(puppet.driven_by.as_ref(), &puppet.name, puppet.at.as_ref()), AnimatedValueType::Float)?;
    if puppet.keyframes.is_empty() {
        return Err(TransformErrorKind::NoKeyframes { layer: puppet.name.clone() }.at(puppet.at.clone()));
    }

    let mut keyframes: Vec<&PuppetKeyframe> = puppet.keyframes.iter().collect();
    keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));
    if let Some(pair) = keyframes.windows(2).find(|pair| pair[0].time == pair[1].time) {
        return Err(TransformErrorKind::DuplicateKeyframe {
            layer: puppet.name.clone(),
            time: pair[0].time,
        }
        .at(puppet.at.clone()));
    }

    let keys: BTreeSet<_> = keyframes.iter().flat_map(|keyframe| keyframe.animation.keys().cloned()).collect();
    for key in &keys {
        let mut written = keyframes.iter().filter_map(|keyframe| keyframe.animation.get(key));
        let first = written.next().expect("every key comes from a keyframe").value.value_type();
        if let Some(other) = written.map(|entry| entry.value.value_type()).find(|found| *found != first) {
            return Err(TransformErrorKind::KeyframeTypeMismatch {
                layer: puppet.name.clone(),
                target: describe(key),
                first,
                second: other,
            }
            .at(puppet.at.clone()));
        }
    }

    let mut fields = Vec::new();
    for (position, keyframe) in keyframes.iter().enumerate() {
        let mut animation = Animation::new();
        for key in &keys {
            let value = match keyframe.animation.get(key) {
                Some(entry) => entry.value.clone(),
                None => filled(key, &keyframes, position),
            };
            animation.insert(FixedAnimationEntry { key: key.clone(), value });
        }
        fields.push(ParametricField {
            position: [keyframe.time, 0.0],
            speed: 1.0,
            motion: Motion::Clip(Clip::Inline(InlineAnimation::Fixed(context.animation(&animation)?))),
        });
    }

    let tree = ParametricBlendTree {
        tree_type: BlendTreeType::Linear,
        x: parameter.clone(),
        y: None,
        fields,
    };
    Ok((parameter, tree, keys))
}

/// The value of a target at a keyframe that does not write it: interpolated between its neighbours, or held at the ends.
fn filled(key: &AnimatedTarget<Declared>, keyframes: &[&PuppetKeyframe], position: usize) -> AnimatedValue<Unresolved<AssetLocator>> {
    fn sample<'a>(key: &AnimatedTarget<Declared>, keyframe: &'a PuppetKeyframe) -> Option<(f64, &'a AnimatedValue<Unresolved<AssetLocator>>)> {
        keyframe.animation.get(key).map(|entry| (keyframe.time, &entry.value))
    }
    let previous = keyframes[..position].iter().rev().find_map(|keyframe| sample(key, keyframe));
    let next = keyframes[position + 1..].iter().find_map(|keyframe| sample(key, keyframe));
    match (previous, next) {
        (Some((from, a)), Some((to, b))) => {
            let t = (keyframes[position].time - from) / (to - from);
            a.lerp(b, t).unwrap_or_else(|| a.clone())
        }
        (Some((_, a)), None) => a.clone(),
        (None, Some((_, b))) => b.clone(),
        (None, None) => unreachable!("the key was taken from one of the keyframes"),
    }
}

fn blend(context: &mut Context, blend: &BlendLayer) -> Result<AnimatorLayer, TransformError> {
    let mut animated = BTreeSet::new();
    let mut fields = Vec::new();
    for puppet in &blend.puppets {
        let (_, tree, keys) = puppet_tree(context, puppet).map_err(|error| error.or_at(puppet.at.as_ref()))?;
        if let Some(target) = keys.iter().find(|key| animated.contains(*key)) {
            return Err(TransformErrorKind::OverlappingBlendTargets {
                layer: blend.name.clone(),
                target: describe(target),
            }
            .at(puppet.at.clone()));
        }
        animated.extend(keys);

        let weight_by = context
            .parameters
            .generate(weight_parameter(&blend.name, &puppet.name), AnimatedValue::Float(1.0))
            .map_err(|error| error.or_at(puppet.at.as_ref()))?;
        fields.push(DirectField {
            weight_by,
            speed: 1.0,
            motion: Motion::BlendTree(BlendTree::Parametric(tree)),
        });
    }

    Ok(AnimatorLayer {
        name: blend.name.clone(),
        default_state: Some(0),
        states: vec![AnimatorState {
            name: blend.name.clone(),
            motion: Some(Motion::BlendTree(BlendTree::Direct(DirectBlendTree { fields }))),
            playback: Playback::default(),
            write_defaults: true,
            behaviors: vec![],
        }],
        transitions: vec![],
    })
}

/// Name of the float parameter that weights one child of a blend layer.
pub(crate) fn weight_parameter(blend: &str, puppet: &str) -> String {
    format!("{blend}/{puppet}")
}

#[cfg(test)]
pub(crate) fn located_at(line: u32) -> Option<crate::core::resolution::SourceLocation> {
    Some(crate::core::resolution::SourceLocation {
        chunk: "avatar.lua".into(),
        line,
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        core::{external::ExternTable, phase::Compiled, resolution::Resolved},
        decl::{
            Avatar,
            behavior::{Behavior, Drive},
            controller::Controller,
            layer::GroupOption,
            parameter::{Parameter, PrimitiveParameter, PrimitiveParameterValue},
        },
        unity::{
            animator::{AnimatedGameObjectProperty, AnimatedGameObjectTarget, AnimatedRendererProperty, AnimatedRendererTarget},
            external::ObjectPath,
        },
        vrchat::playable_layer::PlayableLayer,
    };

    fn parameter(name: &str, value: PrimitiveParameterValue) -> Parameter {
        Parameter::Primitive(PrimitiveParameter {
            name: name.into(),
            value,
            scope: None,
            save: None,
            at: None,
        })
    }

    fn context(layers: Vec<Layer>) -> Context {
        let declaration = Avatar {
            parameters: vec![
                parameter("Emote", PrimitiveParameterValue::Int { default: None, width: None }),
                parameter("Hat", PrimitiveParameterValue::Bool { default: None }),
                parameter("Wink", PrimitiveParameterValue::Float { default: None, width: None }),
                parameter("Brow", PrimitiveParameterValue::Float { default: None, width: None }),
            ],
            controllers: vec![Controller::new(PlayableLayer::Fx, layers)],
            ..Avatar::default()
        };
        let (context, errors) = Context::collect(&declaration);
        assert!(errors.is_empty(), "{errors:?}");
        context
    }

    fn resolved(name: &str, value_type: AnimatedValueType) -> ParameterRef {
        Resolved::new(name.into(), value_type)
    }

    fn shape(name: &str, value: f64) -> FixedAnimationEntry<Declared> {
        FixedAnimationEntry {
            key: AnimatedTarget::Renderer(AnimatedRendererTarget {
                path: Unresolved::new("Face".into()),
                renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
                property: AnimatedRendererProperty::BlendShape { name: name.into() },
            }),
            value: AnimatedValue::Float(value),
        }
    }

    fn material(slot: u32, name: &str) -> FixedAnimationEntry<Declared> {
        FixedAnimationEntry {
            key: AnimatedTarget::Renderer(AnimatedRendererTarget {
                path: Unresolved::new("Body".into()),
                renderer_type: "UnityEngine.SkinnedMeshRenderer".into(),
                property: AnimatedRendererProperty::Material { slot },
            }),
            value: AnimatedValue::ObjectReference(Unresolved::new(AssetLocator::Named {
                asset_type: "UnityEngine.Material".into(),
                name: name.into(),
            })),
        }
    }

    fn active(path: &str, value: bool) -> FixedAnimationEntry<Declared> {
        FixedAnimationEntry {
            key: AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: Unresolved::new(path.into()),
                property: AnimatedGameObjectProperty::Active,
            }),
            value: AnimatedValue::Bool(value),
        }
    }

    fn content(entries: impl IntoIterator<Item = FixedAnimationEntry<Declared>>) -> Content {
        Content {
            animation: Animation::from(entries),
            behaviors: vec![],
        }
    }

    fn option(name: &str, entries: impl IntoIterator<Item = FixedAnimationEntry<Declared>>) -> GroupOption {
        GroupOption {
            name: name.into(),
            content: content(entries),
            at: None,
        }
    }

    type Written = Vec<(String, crate::transform::animation::CompiledValue)>;

    /// The fixed values a state writes, keyed by the blend shape name, for assertions that ignore interning.
    fn shapes_of(state: &AnimatorState) -> Written {
        let Some(Motion::Clip(Clip::Inline(InlineAnimation::Fixed(animation)))) = &state.motion else {
            panic!("expected a fixed clip");
        };
        animation
            .entries()
            .map(|(key, entry)| {
                let name = match key {
                    AnimatedTarget::Renderer(AnimatedRendererTarget {
                        property: AnimatedRendererProperty::BlendShape { name },
                        ..
                    }) => name.clone(),
                    other => format!("{other:?}"),
                };
                (name, entry.value.clone())
            })
            .collect()
    }

    fn expressions(symmetric: Option<bool>) -> GroupLayer {
        GroupLayer {
            name: "Expressions".into(),
            driven_by: Some("Emote".to_owned().into()),
            symmetric,
            default: Some(content([shape("eyelid", 0.3)])),
            options: vec![
                option("smile", [shape("smile", 1.0)]),
                option("angry", [shape("angry", 1.0), shape("eyelid", 0.8)]),
            ],
            at: located_at(10),
        }
    }

    #[rstest]
    fn a_group_layer_completes_its_states_mutually() {
        let layer = expressions(None);
        let compiled = compile(&mut context(vec![Layer::Group(layer.clone())]), &Layer::Group(layer)).unwrap();

        assert_eq!(compiled.name, "Expressions");
        assert_eq!(compiled.default_state, Some(0));
        assert_eq!(
            compiled.states.iter().map(|state| state.name.clone()).collect::<Vec<_>>(),
            ["Default", "smile", "angry"]
        );
        assert_eq!(
            shapes_of(&compiled.states[0]),
            vec![
                ("angry".into(), AnimatedValue::Float(0.0)),
                ("eyelid".into(), AnimatedValue::Float(0.3)),
                ("smile".into(), AnimatedValue::Float(0.0)),
            ]
        );
        assert_eq!(
            shapes_of(&compiled.states[1]),
            vec![
                ("angry".into(), AnimatedValue::Float(0.0)),
                ("eyelid".into(), AnimatedValue::Float(0.3)),
                ("smile".into(), AnimatedValue::Float(1.0)),
            ]
        );
        assert_eq!(
            shapes_of(&compiled.states[2]),
            vec![
                ("angry".into(), AnimatedValue::Float(1.0)),
                ("eyelid".into(), AnimatedValue::Float(0.8)),
                ("smile".into(), AnimatedValue::Float(0.0)),
            ]
        );
    }

    #[rstest]
    fn a_symmetric_group_layer_fans_out_from_entry_and_exits() {
        let layer = expressions(None);
        let compiled = compile(&mut context(vec![Layer::Group(layer.clone())]), &Layer::Group(layer)).unwrap();
        let emote = resolved("Emote", AnimatedValueType::Int);

        assert_eq!(
            compiled.transitions,
            vec![
                transition(
                    TransitionSource::Entry,
                    TransitionTarget::State(1),
                    vec![AnimatorCondition::Equals(emote.clone(), 1)]
                ),
                transition(
                    TransitionSource::Entry,
                    TransitionTarget::State(2),
                    vec![AnimatorCondition::Equals(emote.clone(), 2)]
                ),
                transition(
                    TransitionSource::State(1),
                    TransitionTarget::Exit,
                    vec![AnimatorCondition::NotEqual(emote.clone(), 1)]
                ),
                transition(
                    TransitionSource::State(2),
                    TransitionTarget::Exit,
                    vec![AnimatorCondition::NotEqual(emote.clone(), 2)]
                ),
                transition(
                    TransitionSource::State(0),
                    TransitionTarget::Exit,
                    vec![AnimatorCondition::Equals(emote.clone(), 1)]
                ),
                transition(TransitionSource::State(0), TransitionTarget::Exit, vec![AnimatorCondition::Equals(emote, 2)]),
            ]
        );
    }

    #[rstest]
    fn a_hub_group_layer_goes_through_its_default_state() {
        let layer = expressions(Some(false));
        let compiled = compile(&mut context(vec![Layer::Group(layer.clone())]), &Layer::Group(layer)).unwrap();
        let emote = resolved("Emote", AnimatedValueType::Int);

        assert_eq!(
            compiled.transitions,
            vec![
                transition(
                    TransitionSource::State(0),
                    TransitionTarget::State(1),
                    vec![AnimatorCondition::Equals(emote.clone(), 1)]
                ),
                transition(
                    TransitionSource::State(1),
                    TransitionTarget::State(0),
                    vec![AnimatorCondition::NotEqual(emote.clone(), 1)]
                ),
                transition(
                    TransitionSource::State(0),
                    TransitionTarget::State(2),
                    vec![AnimatorCondition::Equals(emote.clone(), 2)]
                ),
                transition(
                    TransitionSource::State(2),
                    TransitionTarget::State(0),
                    vec![AnimatorCondition::NotEqual(emote, 2)]
                ),
            ]
        );
    }

    #[rstest]
    fn a_group_layer_without_a_driver_follows_a_parameter_of_its_own_name() {
        let layer = GroupLayer {
            name: "Emote".into(),
            driven_by: None,
            ..expressions(None)
        };
        let compiled = compile(&mut context(vec![Layer::Group(layer.clone())]), &Layer::Group(layer)).unwrap();
        assert!(matches!(&compiled.transitions[0].conditions[0], AnimatorCondition::Equals(parameter, 1) if parameter.value == "Emote"));
    }

    #[rstest]
    fn a_group_layer_needs_an_int_driver() {
        let layer = GroupLayer {
            driven_by: Some(Unresolved::located("Hat".into(), located_at(11).unwrap())),
            ..expressions(None)
        };
        let error = compile(&mut context(vec![Layer::Group(layer.clone())]), &Layer::Group(layer)).unwrap_err();
        assert_eq!(
            error,
            TransformErrorKind::ParameterTypeMismatch {
                name: "Hat".into(),
                expected: AnimatedValueType::Int,
                found: AnimatedValueType::Bool,
            }
            .at(located_at(11))
        );
    }

    #[rstest]
    fn an_option_may_not_introduce_a_reference_the_default_lacks() {
        let layer = GroupLayer {
            options: vec![option("dressed", [material(0, "Dressed")])],
            ..expressions(None)
        };
        let error = compile(&mut context(vec![Layer::Group(layer.clone())]), &Layer::Group(layer)).unwrap_err();
        assert_eq!(
            error,
            TransformErrorKind::OptionsNeedDefault {
                layer: "Expressions".into(),
                targets: "`Body` material 0".into(),
            }
            .at(located_at(10))
        );
    }

    #[rstest]
    fn option_behaviors_are_compiled_into_the_state() {
        let mut layer = expressions(None);
        layer.options[0].content.behaviors.push(Behavior::Drive(Drive::Parameter {
            parameter: "Hat".to_owned().into(),
            value: AnimatedValue::Bool(true),
        }));
        let compiled = compile(&mut context(vec![Layer::Group(layer.clone())]), &Layer::Group(layer)).unwrap();
        assert_eq!(compiled.states[1].behaviors.len(), 1);
        assert!(compiled.states[0].behaviors.is_empty());
    }

    fn hat(content: SwitchContent) -> SwitchLayer {
        SwitchLayer {
            name: "Hat".into(),
            driven_by: None,
            content,
            at: located_at(20),
        }
    }

    #[rstest]
    fn a_toggle_list_zeroes_itself_for_the_disabled_side() {
        let layer = hat(SwitchContent::Toggle(content([active("Hat", true), shape("hat_hair", 1.0)])));
        let compiled = compile(&mut context(vec![Layer::Switch(layer.clone())]), &Layer::Switch(layer)).unwrap();
        let parameter = resolved("Hat", AnimatedValueType::Bool);

        assert_eq!(compiled.default_state, Some(0));
        assert_eq!(
            compiled.states.iter().map(|state| state.name.clone()).collect::<Vec<_>>(),
            ["Disabled", "Enabled"]
        );
        let values = |state: &AnimatorState| shapes_of(state).into_iter().map(|(_, value)| value).collect::<Vec<_>>();
        assert_eq!(values(&compiled.states[0]), [AnimatedValue::Bool(false), AnimatedValue::Float(0.0)]);
        assert_eq!(values(&compiled.states[1]), [AnimatedValue::Bool(true), AnimatedValue::Float(1.0)]);
        assert_eq!(
            compiled.transitions,
            vec![
                transition(
                    TransitionSource::State(0),
                    TransitionTarget::State(1),
                    vec![AnimatorCondition::If(parameter.clone())]
                ),
                transition(
                    TransitionSource::State(1),
                    TransitionTarget::State(0),
                    vec![AnimatorCondition::IfNot(parameter)]
                ),
            ]
        );
    }

    #[rstest]
    fn both_sides_are_taken_as_written() {
        let layer = hat(SwitchContent::Sides {
            off: content([]),
            on: content([material(0, "Dressed")]),
        });
        let compiled = compile(&mut context(vec![Layer::Switch(layer.clone())]), &Layer::Switch(layer)).unwrap();
        assert!(shapes_of(&compiled.states[0]).is_empty());
        assert_eq!(shapes_of(&compiled.states[1]).len(), 1);
    }

    #[rstest]
    fn a_toggle_list_cannot_zero_a_reference() {
        let layer = hat(SwitchContent::Toggle(content([material(0, "Dressed")])));
        let error = compile(&mut context(vec![Layer::Switch(layer.clone())]), &Layer::Switch(layer)).unwrap_err();
        assert_eq!(
            error,
            TransformErrorKind::ToggleNeedsBothSides {
                layer: "Hat".into(),
                target: "`Body` material 0".into(),
            }
            .at(located_at(20))
        );
    }

    fn keyframe(time: f64, entries: impl IntoIterator<Item = FixedAnimationEntry<Declared>>) -> PuppetKeyframe {
        PuppetKeyframe {
            time,
            animation: Animation::from(entries),
        }
    }

    fn wink(keyframes: Vec<PuppetKeyframe>) -> PuppetLayer {
        PuppetLayer {
            name: "Wink".into(),
            driven_by: None,
            keyframes,
            at: located_at(30),
        }
    }

    fn fields_of(layer: &AnimatorLayer) -> Vec<(f64, Written)> {
        let Some(Motion::BlendTree(BlendTree::Parametric(tree))) = &layer.states[0].motion else {
            panic!("expected a parametric blend tree");
        };
        assert_eq!(tree.tree_type, BlendTreeType::Linear);
        assert!(tree.y.is_none());
        tree.fields
            .iter()
            .map(|field| {
                let state = AnimatorState {
                    name: String::new(),
                    motion: Some(field.motion.clone()),
                    playback: Playback::default(),
                    write_defaults: false,
                    behaviors: vec![],
                };
                (field.position[0], shapes_of(&state))
            })
            .collect()
    }

    #[rstest]
    fn a_puppet_layer_interpolates_missing_targets_and_holds_the_ends() {
        let layer = wink(vec![
            keyframe(1.0, [shape("wink_r", 1.0)]),
            keyframe(-1.0, [shape("wink_l", 1.0), shape("brow", 0.2)]),
            keyframe(0.0, [shape("brow", 0.6)]),
        ]);
        let compiled = compile(&mut context(vec![Layer::Puppet(layer.clone())]), &Layer::Puppet(layer)).unwrap();

        let Some(Motion::BlendTree(BlendTree::Parametric(tree))) = &compiled.states[0].motion else {
            panic!("expected a parametric blend tree");
        };
        assert_eq!(tree.x, resolved("Wink", AnimatedValueType::Float));
        assert_eq!(
            fields_of(&compiled),
            vec![
                (
                    -1.0,
                    vec![
                        ("brow".into(), AnimatedValue::Float(0.2)),
                        ("wink_l".into(), AnimatedValue::Float(1.0)),
                        ("wink_r".into(), AnimatedValue::Float(1.0)),
                    ]
                ),
                (
                    0.0,
                    vec![
                        ("brow".into(), AnimatedValue::Float(0.6)),
                        ("wink_l".into(), AnimatedValue::Float(1.0)),
                        ("wink_r".into(), AnimatedValue::Float(1.0)),
                    ]
                ),
                (
                    1.0,
                    vec![
                        ("brow".into(), AnimatedValue::Float(0.6)),
                        ("wink_l".into(), AnimatedValue::Float(1.0)),
                        ("wink_r".into(), AnimatedValue::Float(1.0)),
                    ]
                ),
            ]
        );
    }

    #[rstest]
    fn a_puppet_layer_interpolates_between_written_neighbours() {
        let layer = wink(vec![
            keyframe(0.0, [shape("open", 0.0), active("Hat", false)]),
            keyframe(0.5, []),
            keyframe(2.0, [shape("open", 1.0), active("Hat", true)]),
        ]);
        let compiled = compile(&mut context(vec![Layer::Puppet(layer.clone())]), &Layer::Puppet(layer)).unwrap();
        let middle = &fields_of(&compiled)[1];
        assert_eq!(middle.0, 0.5);
        assert_eq!(middle.1[0].1, AnimatedValue::Bool(false));
        assert_eq!(middle.1[1].1, AnimatedValue::Float(0.25));
    }

    #[rstest]
    #[case::empty(vec![], TransformErrorKind::NoKeyframes { layer: "Wink".into() })]
    #[case::duplicate(
        vec![keyframe(0.0, []), keyframe(1.0, []), keyframe(0.0, [])],
        TransformErrorKind::DuplicateKeyframe { layer: "Wink".into(), time: 0.0 },
    )]
    #[case::mixed(
        vec![keyframe(0.0, [shape("open", 0.0)]), keyframe(1.0, [FixedAnimationEntry { value: AnimatedValue::Int(1), ..shape("open", 0.0) }])],
        TransformErrorKind::KeyframeTypeMismatch {
            layer: "Wink".into(),
            target: "`Face` shape `open`".into(),
            first: AnimatedValueType::Float,
            second: AnimatedValueType::Int,
        },
    )]
    fn a_bad_puppet_layer_is_reported(#[case] keyframes: Vec<PuppetKeyframe>, #[case] expected: TransformErrorKind) {
        let layer = wink(keyframes);
        let error = compile(&mut context(vec![Layer::Puppet(layer.clone())]), &Layer::Puppet(layer)).unwrap_err();
        assert_eq!(error, expected.at(located_at(30)));
    }

    fn face(puppets: Vec<PuppetLayer>) -> BlendLayer {
        BlendLayer {
            name: "Face".into(),
            puppets,
            at: located_at(40),
        }
    }

    #[rstest]
    fn a_blend_layer_merges_its_children_into_a_direct_tree() {
        let layer = face(vec![
            wink(vec![keyframe(0.0, [shape("wink", 0.0)]), keyframe(1.0, [shape("wink", 1.0)])]),
            PuppetLayer {
                name: "Brow".into(),
                keyframes: vec![keyframe(0.0, [shape("brow", 0.0)]), keyframe(1.0, [shape("brow", 1.0)])],
                ..wink(vec![])
            },
        ]);
        let mut context = context(vec![Layer::Blend(layer.clone())]);
        let compiled = compile(&mut context, &Layer::Blend(layer)).unwrap();

        assert_eq!(compiled.states.len(), 1);
        assert!(compiled.states[0].write_defaults);
        assert!(compiled.transitions.is_empty());
        let Some(Motion::BlendTree(BlendTree::Direct(tree))) = &compiled.states[0].motion else {
            panic!("expected a direct blend tree");
        };
        assert_eq!(
            tree.fields.iter().map(|field| field.weight_by.clone()).collect::<Vec<_>>(),
            [resolved("Face/Wink", AnimatedValueType::Float), resolved("Face/Brow", AnimatedValueType::Float)]
        );
        assert!(
            tree.fields
                .iter()
                .all(|field| matches!(field.motion, Motion::BlendTree(BlendTree::Parametric(_))))
        );

        let generated: Vec<_> = context
            .parameters
            .animator_parameters()
            .into_iter()
            .filter(|parameter| parameter.name.starts_with("Face/"))
            .collect();
        assert_eq!(generated.len(), 2);
        assert!(generated.iter().all(|parameter| parameter.default_value == Some(1.0)));
        assert!(
            context
                .parameters
                .expression_parameters()
                .iter()
                .all(|parameter| !parameter.name.starts_with("Face/"))
        );
    }

    #[rstest]
    fn children_of_a_blend_layer_may_not_share_a_target() {
        let layer = face(vec![
            wink(vec![keyframe(0.0, [shape("wink", 0.0)])]),
            PuppetLayer {
                name: "Brow".into(),
                keyframes: vec![keyframe(0.0, [shape("wink", 1.0)])],
                at: located_at(42),
                ..wink(vec![])
            },
        ]);
        let error = compile(&mut context(vec![Layer::Blend(layer.clone())]), &Layer::Blend(layer)).unwrap_err();
        assert_eq!(
            error,
            TransformErrorKind::OverlappingBlendTargets {
                layer: "Face".into(),
                target: "`Face` shape `wink`".into(),
            }
            .at(located_at(42))
        );
    }

    #[rstest]
    fn a_layer_error_without_a_location_points_at_the_layer() {
        let layer = wink(vec![]);
        let error = compile(&mut context(vec![Layer::Puppet(layer.clone())]), &Layer::Puppet(layer)).unwrap_err();
        assert_eq!(error.at, located_at(30));
    }

    #[rstest]
    fn compiled_targets_index_the_object_path_table() {
        let layer = hat(SwitchContent::Toggle(content([active("Hat", true)])));
        let mut context = context(vec![Layer::Switch(layer.clone())]);
        let compiled = compile(&mut context, &Layer::Switch(layer)).unwrap();

        let Some(Motion::Clip(Clip::Inline(InlineAnimation::Fixed(animation)))) = &compiled.states[1].motion else {
            panic!("expected a fixed clip");
        };
        let (AnimatedTarget::<Compiled>::GameObject(target), _) = animation.entries().next().unwrap() else {
            panic!("expected a game object target");
        };
        let paths: &ExternTable<ObjectPath> = &context.externals.object_paths;
        assert_eq!(paths.get(target.path).value, "Hat");
    }
}
