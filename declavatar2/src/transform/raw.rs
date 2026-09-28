use std::collections::BTreeMap;

use crate::{
    avatar::controller::{
        AnimatorCondition, AnimatorLayer, AnimatorState, AnimatorTransition, BlendTree, Clip, DirectBlendTree, DirectField, Motion, ParametricBlendTree,
        ParametricField, Playback, StateMachine, TransitionSource, TransitionTarget,
    },
    core::resolution::{SourceLocation, Unresolved},
    decl::raw::{self, ClipOptions, Condition, RawMachine},
    transform::{
        behavior::cast,
        context::Context,
        error::{TransformError, TransformErrorKind},
    },
    unity::{animation::InlineAnimation, value::AnimatedValue, value::AnimatedValueType},
};

pub(crate) fn compile(context: &mut Context, raw: &RawMachine) -> Result<AnimatorLayer, TransformError> {
    let mut layer = AnimatorLayer {
        name: raw.name.clone(),
        default_state: None,
        machines: Vec::new(),
        states: Vec::new(),
        transitions: Vec::new(),
    };
    layer.default_state = machine(context, &mut layer, raw, None, &raw.name)?;
    Ok(layer)
}

/// Something a machine holds directly, which a name written in that machine refers to.
#[derive(Debug, Clone, Copy)]
enum Node {
    State(usize),
    Machine(usize),
}

/// Compiles the states, nested machines and transitions of one machine into `layer` and returns its default state.
fn machine(context: &mut Context, layer: &mut AnimatorLayer, raw: &RawMachine, id: Option<usize>, path: &str) -> Result<Option<usize>, TransformError> {
    let mut scope = BTreeMap::new();
    let mut declare = |name: &str, node: Node, at: Option<&SourceLocation>| {
        if scope.insert(name.to_owned(), node).is_some() {
            return Err(TransformErrorKind::DuplicateState {
                machine: path.to_owned(),
                name: name.to_owned(),
            }
            .at(at.cloned()));
        }
        Ok(())
    };

    let first_state = (!raw.states.is_empty()).then_some(layer.states.len());
    for written in &raw.states {
        declare(&written.name, Node::State(layer.states.len()), written.at.as_ref())?;
        let (motion, playback) = match &written.motion {
            Some(motion) => {
                let (motion, playback) = state_motion(context, motion).map_err(|error| error.or_at(written.at.as_ref()))?;
                (Some(motion), playback)
            }
            None => (None, Playback::default()),
        };
        layer.states.push(AnimatorState {
            name: written.name.clone(),
            machine: id,
            motion,
            playback,
            write_defaults: false,
            behaviors: context.behaviors(&written.behaviors).map_err(|error| error.or_at(written.at.as_ref()))?,
        });
    }

    for nested in &raw.machines {
        let index = layer.machines.len();
        declare(&nested.name, Node::Machine(index), nested.at.as_ref())?;
        layer.machines.push(StateMachine {
            name: nested.name.clone(),
            parent: id,
            default_state: None,
        });
        let nested_path = format!("{path}/{}", nested.name);
        let default_state = machine(context, layer, nested, Some(index), &nested_path)?;
        if default_state.is_none() {
            return Err(TransformErrorKind::MachineWithoutState { machine: nested_path }.at(nested.at.clone()));
        }
        layer.machines[index].default_state = default_state;
    }

    let lookup = |reference: &Unresolved<String>| {
        scope.get(&reference.value).copied().ok_or_else(|| {
            TransformErrorKind::UnknownState {
                machine: path.to_owned(),
                name: reference.value.clone(),
            }
            .at(reference.at.clone())
        })
    };

    let default_state = match &raw.default_state {
        Some(reference) => match lookup(reference)? {
            Node::State(index) => Some(index),
            Node::Machine(_) => {
                return Err(TransformErrorKind::DefaultIsMachine {
                    machine: path.to_owned(),
                    name: reference.value.clone(),
                }
                .at(reference.at.clone()));
            }
        },
        None => first_state,
    };

    for written in &raw.transitions {
        let immediate = |leaving: String, at: Option<&SourceLocation>| match written.duration {
            Some(_) => Err(TransformErrorKind::ImmediateTransitionDuration { leaving }.at(at.or(raw.at.as_ref()).cloned())),
            None => Ok(()),
        };
        let from = match &written.from {
            raw::TransitionSource::Entry => {
                immediate(format!("the entry of `{path}`"), None)?;
                TransitionSource::Entry(id)
            }
            raw::TransitionSource::Node(reference) => match lookup(reference)? {
                Node::State(index) => TransitionSource::State(index),
                Node::Machine(index) => {
                    immediate(format!("state machine `{path}/{}`", reference.value), reference.at.as_ref())?;
                    TransitionSource::MachineExit(index)
                }
            },
        };
        let to = match &written.to {
            raw::TransitionTarget::Exit => TransitionTarget::Exit,
            raw::TransitionTarget::Node(reference) => match lookup(reference)? {
                Node::State(index) => TransitionTarget::State(index),
                Node::Machine(index) => TransitionTarget::Machine(index),
            },
        };
        if let (TransitionSource::Entry(_), TransitionTarget::Exit) = (from, to) {
            return Err(TransformErrorKind::EntryLeadsToExit { machine: path.to_owned() }.at(raw.at.clone()));
        }
        layer.transitions.push(AnimatorTransition {
            from,
            to,
            duration: written.duration.unwrap_or(0.0),
            conditions: written
                .conditions
                .iter()
                .map(|condition| self::condition(context, condition))
                .collect::<Result<_, _>>()?,
        });
    }

    Ok(default_state)
}

/// The motion of a state, with the clip options moved onto the state where Unity keeps them.
fn state_motion(context: &mut Context, motion: &raw::Motion) -> Result<(Motion, Playback), TransformError> {
    Ok(match motion {
        raw::Motion::Clip { options, animation } => (
            Motion::Clip(Clip::Inline(InlineAnimation::Fixed(context.animation(animation)?))),
            playback(context, options)?,
        ),
        raw::Motion::Keyed { options, animation } => (
            Motion::Clip(Clip::Inline(InlineAnimation::Keyed(context.keyed_animation(animation)?))),
            playback(context, options)?,
        ),
        raw::Motion::External { asset, options } => (
            Motion::Clip(Clip::External(context.externals.assets.intern(asset.clone()))),
            playback(context, options)?,
        ),
        raw::Motion::BlendTree(tree) => (Motion::BlendTree(blend_tree(context, tree)?), Playback::default()),
    })
}

fn playback(context: &Context, options: &ClipOptions) -> Result<Playback, TransformError> {
    let float = |parameter: &Option<Unresolved<String>>| {
        parameter
            .as_ref()
            .map(|parameter| context.parameters.resolve_typed(parameter, AnimatedValueType::Float))
            .transpose()
    };
    Ok(Playback {
        speed: options.speed.unwrap_or(1.0),
        speed_by: float(&options.speed_by)?,
        time_by: float(&options.time_by)?,
    })
}

/// A motion inside a blend tree, which can scale its speed but cannot follow a parameter.
fn nested_motion(context: &mut Context, motion: &raw::Motion) -> Result<(Motion, f64), TransformError> {
    let nested = |options: &ClipOptions| {
        let rejected = |option: &'static str, at: Option<&SourceLocation>| TransformErrorKind::NestedPlayback { option }.at(at.cloned());
        if let Some(parameter) = &options.speed_by {
            return Err(rejected("speed_by", parameter.at.as_ref()));
        }
        if let Some(parameter) = &options.time_by {
            return Err(rejected("time_by", parameter.at.as_ref()));
        }
        Ok(options.speed.unwrap_or(1.0))
    };
    Ok(match motion {
        raw::Motion::Clip { options, animation } => (
            Motion::Clip(Clip::Inline(InlineAnimation::Fixed(context.animation(animation)?))),
            nested(options)?,
        ),
        raw::Motion::Keyed { options, animation } => (
            Motion::Clip(Clip::Inline(InlineAnimation::Keyed(context.keyed_animation(animation)?))),
            nested(options)?,
        ),
        raw::Motion::External { asset, options } => (Motion::Clip(Clip::External(context.externals.assets.intern(asset.clone()))), nested(options)?),
        raw::Motion::BlendTree(tree) => (Motion::BlendTree(blend_tree(context, tree)?), 1.0),
    })
}

fn blend_tree(context: &mut Context, tree: &raw::BlendTree) -> Result<BlendTree, TransformError> {
    Ok(match tree {
        raw::BlendTree::Parametric(parametric) => {
            let x = context.parameters.resolve_typed(&parametric.x, AnimatedValueType::Float)?;
            let y = parametric
                .y
                .as_ref()
                .map(|parameter| context.parameters.resolve_typed(parameter, AnimatedValueType::Float))
                .transpose()?;
            let mut fields = Vec::new();
            for field in &parametric.fields {
                let (motion, speed) = nested_motion(context, &field.motion)?;
                fields.push(ParametricField {
                    position: field.position,
                    speed,
                    motion,
                });
            }
            BlendTree::Parametric(ParametricBlendTree {
                tree_type: parametric.tree_type,
                x,
                y,
                fields,
            })
        }
        raw::BlendTree::Direct(direct) => {
            let mut fields = Vec::new();
            for field in &direct.fields {
                let weight_by = context.parameters.resolve_typed(&field.weight_by, AnimatedValueType::Float)?;
                let (motion, speed) = nested_motion(context, &field.motion)?;
                fields.push(DirectField { weight_by, speed, motion });
            }
            BlendTree::Direct(DirectBlendTree { fields })
        }
    })
}

/// Turns a written condition into the comparison Unity supports for the parameter's type.
fn condition(context: &Context, condition: &Condition) -> Result<AnimatorCondition, TransformError> {
    let reference = condition.parameter();
    let parameter = context.parameters.resolve(reference)?;
    let value_type = parameter.context;
    let at = reference.at.as_ref();

    let unsupported = |name: &'static str| {
        TransformErrorKind::UnsupportedCondition {
            condition: name,
            parameter: parameter.value.clone(),
            value_type,
        }
        .at(at.cloned())
    };
    let bool_of = |value: &AnimatedValue<()>| -> Result<bool, TransformError> {
        match cast(value, AnimatedValueType::Bool).map_err(|error| error.or_at(at))? {
            AnimatedValue::Bool(value) => Ok(value),
            _ => unreachable!("a cast to bool gives a bool"),
        }
    };
    let int_of = |value: &AnimatedValue<()>| -> Result<i64, TransformError> {
        match cast(value, AnimatedValueType::Int).map_err(|error| error.or_at(at))? {
            AnimatedValue::Int(value) => Ok(value),
            _ => unreachable!("a cast to int gives an int"),
        }
    };
    let float_of = |value: &AnimatedValue<()>| -> Result<f64, TransformError> {
        match cast(value, AnimatedValueType::Float).map_err(|error| error.or_at(at))? {
            AnimatedValue::Float(value) => Ok(value),
            _ => unreachable!("a cast to float gives a float"),
        }
    };

    Ok(match (condition, value_type) {
        (Condition::Zero(_), AnimatedValueType::Bool) => AnimatorCondition::IfNot(parameter),
        (Condition::Zero(_), AnimatedValueType::Int) => AnimatorCondition::Equals(parameter, 0),
        (Condition::NonZero(_), AnimatedValueType::Bool) => AnimatorCondition::If(parameter),
        (Condition::NonZero(_), AnimatedValueType::Int) => AnimatorCondition::NotEqual(parameter, 0),
        (Condition::Eq(_, value), AnimatedValueType::Bool) => match bool_of(value)? {
            true => AnimatorCondition::If(parameter),
            false => AnimatorCondition::IfNot(parameter),
        },
        (Condition::Ne(_, value), AnimatedValueType::Bool) => match bool_of(value)? {
            true => AnimatorCondition::IfNot(parameter),
            false => AnimatorCondition::If(parameter),
        },
        (Condition::Eq(_, value), AnimatedValueType::Int) => AnimatorCondition::Equals(parameter, int_of(value)?),
        (Condition::Ne(_, value), AnimatedValueType::Int) => AnimatorCondition::NotEqual(parameter, int_of(value)?),
        (Condition::Gt(_, value), AnimatedValueType::Int | AnimatedValueType::Float) => AnimatorCondition::Greater(parameter, float_of(value)?),
        (Condition::Lt(_, value), AnimatedValueType::Int | AnimatedValueType::Float) => AnimatorCondition::Less(parameter, float_of(value)?),
        (Condition::Zero(_), _) => return Err(unsupported("zero")),
        (Condition::NonZero(_), _) => return Err(unsupported("nonzero")),
        (Condition::Eq(..), _) => return Err(unsupported("eq")),
        (Condition::Ne(..), _) => return Err(unsupported("ne")),
        (Condition::Gt(..), _) => return Err(unsupported("gt")),
        (Condition::Lt(..), _) => return Err(unsupported("lt")),
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        avatar::controller::ParameterRef,
        core::resolution::Resolved,
        decl::{
            Avatar,
            behavior::Animation,
            parameter::{Parameter, PrimitiveParameter, PrimitiveParameterValue},
            raw::{
                BlendTreeField, BlendTreeType, DirectBlendTreeField, ParametricBlendTree as RawParametricBlendTree, RawState, RawTransition,
                TransitionSource as RawSource, TransitionTarget as RawTarget,
            },
        },
        transform::layer::located_at,
        unity::external::AssetLocator,
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

    fn context() -> Context {
        let declaration = Avatar {
            parameters: vec![
                parameter("Emote", PrimitiveParameterValue::Int { default: None, width: None }),
                parameter("Hat", PrimitiveParameterValue::Bool { default: None }),
                parameter("Blend", PrimitiveParameterValue::Float { default: None, width: None }),
                parameter("Weight", PrimitiveParameterValue::Float { default: None, width: None }),
            ],
            ..Avatar::default()
        };
        Context::collect(&declaration).0
    }

    fn resolved(name: &str, value_type: AnimatedValueType) -> ParameterRef {
        Resolved::new(name.into(), value_type)
    }

    fn located(name: &str, line: u32) -> Unresolved<String> {
        Unresolved::located(name.into(), located_at(line).unwrap())
    }

    fn clip(options: ClipOptions) -> raw::Motion {
        raw::Motion::Clip {
            options,
            animation: Animation::new(),
        }
    }

    fn external(name: &str, options: ClipOptions) -> raw::Motion {
        raw::Motion::External {
            asset: Unresolved::new(AssetLocator::Named {
                asset_type: "UnityEngine.AnimationClip".into(),
                name: name.into(),
            }),
            options,
        }
    }

    fn state(name: &str, motion: Option<raw::Motion>, line: u32) -> RawState {
        RawState {
            name: name.into(),
            motion,
            behaviors: vec![],
            at: located_at(line),
        }
    }

    fn layer(default_state: Option<Unresolved<String>>, states: Vec<RawState>, transitions: Vec<RawTransition>) -> RawMachine {
        RawMachine {
            name: "Raw".into(),
            default_state,
            states,
            machines: vec![],
            transitions,
            at: located_at(50),
        }
    }

    fn nested(name: &str, states: Vec<RawState>, machines: Vec<RawMachine>, transitions: Vec<RawTransition>, line: u32) -> RawMachine {
        RawMachine {
            name: name.into(),
            default_state: None,
            states,
            machines,
            transitions,
            at: located_at(line),
        }
    }

    fn from(name: &str) -> RawSource {
        RawSource::Node(name.to_owned().into())
    }

    fn to(name: &str) -> RawTarget {
        RawTarget::Node(name.to_owned().into())
    }

    fn edge(from: RawSource, to: RawTarget) -> RawTransition {
        RawTransition {
            from,
            to,
            duration: None,
            conditions: vec![],
        }
    }

    #[rstest]
    fn states_and_transitions_are_resolved_by_index() {
        let raw = layer(
            Some("B".to_owned().into()),
            vec![state("A", Some(clip(ClipOptions::default())), 51), state("B", None, 52)],
            vec![RawTransition {
                from: from("A"),
                to: to("B"),
                duration: Some(0.25),
                conditions: vec![Condition::NonZero("Hat".to_owned().into())],
            }],
        );
        let compiled = compile(&mut context(), &raw).unwrap();

        assert_eq!(compiled.name, "Raw");
        assert_eq!(compiled.default_state, Some(1));
        assert_eq!(compiled.states.iter().map(|state| state.name.clone()).collect::<Vec<_>>(), ["A", "B"]);
        assert!(matches!(compiled.states[0].motion, Some(Motion::Clip(Clip::Inline(InlineAnimation::Fixed(_))))));
        assert_eq!(compiled.states[1].motion, None);
        assert_eq!(
            compiled.transitions,
            vec![AnimatorTransition {
                from: TransitionSource::State(0),
                to: TransitionTarget::State(1),
                duration: 0.25,
                conditions: vec![AnimatorCondition::If(resolved("Hat", AnimatedValueType::Bool))],
            }]
        );
    }

    #[rstest]
    fn the_first_state_is_the_default_unless_written() {
        let compiled = compile(&mut context(), &layer(None, vec![state("A", None, 51), state("B", None, 52)], vec![])).unwrap();
        assert_eq!(compiled.default_state, Some(0));

        let empty = compile(&mut context(), &layer(None, vec![], vec![])).unwrap();
        assert_eq!(empty.default_state, None);
    }

    #[rstest]
    fn nested_machines_follow_their_parents_and_hold_their_own_states() {
        let finale = nested("Finale", vec![state("Bow", None, 61)], vec![], vec![edge(from("Bow"), RawTarget::Exit)], 60);
        let dance = nested(
            "Dance",
            vec![state("Step", None, 56)],
            vec![finale],
            vec![edge(RawSource::Entry, to("Finale")), edge(from("Finale"), to("Step"))],
            55,
        );
        let mut raw = layer(
            None,
            vec![state("Idle", None, 51)],
            vec![edge(from("Idle"), to("Dance")), edge(from("Dance"), RawTarget::Exit)],
        );
        raw.machines.push(dance);
        let compiled = compile(&mut context(), &raw).unwrap();

        assert_eq!(compiled.default_state, Some(0));
        assert_eq!(
            compiled.machines,
            vec![
                StateMachine {
                    name: "Dance".into(),
                    parent: None,
                    default_state: Some(1),
                },
                StateMachine {
                    name: "Finale".into(),
                    parent: Some(0),
                    default_state: Some(2),
                },
            ]
        );
        assert_eq!(
            compiled.states.iter().map(|state| (state.name.as_str(), state.machine)).collect::<Vec<_>>(),
            [("Idle", None), ("Step", Some(0)), ("Bow", Some(1))]
        );
        assert_eq!(
            compiled
                .transitions
                .iter()
                .map(|transition| (transition.from, transition.to))
                .collect::<Vec<_>>(),
            [
                (TransitionSource::State(2), TransitionTarget::Exit),
                (TransitionSource::Entry(Some(0)), TransitionTarget::Machine(1)),
                (TransitionSource::MachineExit(1), TransitionTarget::State(1)),
                (TransitionSource::State(0), TransitionTarget::Machine(0)),
                (TransitionSource::MachineExit(0), TransitionTarget::Exit),
            ]
        );
    }

    #[rstest]
    fn names_are_scoped_to_the_machine_holding_them() {
        let loop_in = |name: &str, line: u32| {
            nested(
                name,
                vec![state("Loop", None, line + 1)],
                vec![],
                vec![edge(from("Loop"), RawTarget::Exit)],
                line,
            )
        };
        let mut raw = layer(None, vec![state("Loop", None, 51)], vec![edge(RawSource::Entry, to("A"))]);
        raw.machines = vec![loop_in("A", 55), loop_in("B", 60)];
        let compiled = compile(&mut context(), &raw).unwrap();

        assert_eq!(compiled.states.iter().filter(|state| state.name == "Loop").count(), 3);
        assert_eq!(
            compiled
                .transitions
                .iter()
                .map(|transition| (transition.from, transition.to))
                .collect::<Vec<_>>(),
            [
                (TransitionSource::State(1), TransitionTarget::Exit),
                (TransitionSource::State(2), TransitionTarget::Exit),
                (TransitionSource::Entry(None), TransitionTarget::Machine(0)),
            ]
        );
    }

    #[rstest]
    #[case::duplicate(
        layer(None, vec![state("A", None, 51), state("A", None, 52)], vec![]),
        TransformErrorKind::DuplicateState { machine: "Raw".into(), name: "A".into() }.at(located_at(52)),
    )]
    #[case::unknown_default(
        layer(Some(located("C", 53)), vec![state("A", None, 51)], vec![]),
        TransformErrorKind::UnknownState { machine: "Raw".into(), name: "C".into() }.at(located_at(53)),
    )]
    #[case::unknown_target(
        layer(None, vec![state("A", None, 51)], vec![RawTransition { from: from("A"), to: RawTarget::Node(located("C", 54)), duration: None, conditions: vec![] }]),
        TransformErrorKind::UnknownState { machine: "Raw".into(), name: "C".into() }.at(located_at(54)),
    )]
    #[case::state_of_a_nested_machine(
        RawMachine {
            machines: vec![nested("M", vec![state("Inner", None, 56)], vec![], vec![], 55)],
            ..layer(None, vec![state("A", None, 51)], vec![RawTransition { from: from("A"), to: RawTarget::Node(located("Inner", 57)), duration: None, conditions: vec![] }])
        },
        TransformErrorKind::UnknownState { machine: "Raw".into(), name: "Inner".into() }.at(located_at(57)),
    )]
    #[case::state_of_the_parent(
        RawMachine {
            machines: vec![nested("M", vec![state("Inner", None, 56)], vec![], vec![RawTransition { from: from("Inner"), to: RawTarget::Node(located("A", 57)), duration: None, conditions: vec![] }], 55)],
            ..layer(None, vec![state("A", None, 51)], vec![])
        },
        TransformErrorKind::UnknownState { machine: "Raw/M".into(), name: "A".into() }.at(located_at(57)),
    )]
    #[case::state_and_machine_of_one_name(
        RawMachine {
            machines: vec![nested("A", vec![state("Inner", None, 56)], vec![], vec![], 55)],
            ..layer(None, vec![state("A", None, 51)], vec![])
        },
        TransformErrorKind::DuplicateState { machine: "Raw".into(), name: "A".into() }.at(located_at(55)),
    )]
    #[case::machine_as_default(
        RawMachine {
            machines: vec![nested("M", vec![state("Inner", None, 56)], vec![], vec![], 55)],
            ..layer(Some(located("M", 53)), vec![state("A", None, 51)], vec![])
        },
        TransformErrorKind::DefaultIsMachine { machine: "Raw".into(), name: "M".into() }.at(located_at(53)),
    )]
    #[case::machine_without_state(
        RawMachine {
            machines: vec![nested("M", vec![], vec![nested("N", vec![state("Inner", None, 57)], vec![], vec![], 56)], vec![], 55)],
            ..layer(None, vec![state("A", None, 51)], vec![])
        },
        TransformErrorKind::MachineWithoutState { machine: "Raw/M".into() }.at(located_at(55)),
    )]
    #[case::timed_machine_exit(
        RawMachine {
            machines: vec![nested("M", vec![state("Inner", None, 56)], vec![], vec![], 55)],
            ..layer(None, vec![state("A", None, 51)], vec![RawTransition { from: RawSource::Node(located("M", 58)), to: to("A"), duration: Some(0.5), conditions: vec![] }])
        },
        TransformErrorKind::ImmediateTransitionDuration { leaving: "state machine `Raw/M`".into() }.at(located_at(58)),
    )]
    #[case::timed_entry(
        layer(None, vec![state("A", None, 51)], vec![RawTransition { from: RawSource::Entry, to: to("A"), duration: Some(0.5), conditions: vec![] }]),
        TransformErrorKind::ImmediateTransitionDuration { leaving: "the entry of `Raw`".into() }.at(located_at(50)),
    )]
    #[case::entry_to_exit(
        layer(None, vec![state("A", None, 51)], vec![edge(RawSource::Entry, RawTarget::Exit)]),
        TransformErrorKind::EntryLeadsToExit { machine: "Raw".into() }.at(located_at(50)),
    )]
    fn a_bad_state_reference_is_reported(#[case] raw: RawMachine, #[case] expected: TransformError) {
        assert_eq!(compile(&mut context(), &raw).unwrap_err(), expected);
    }

    #[rstest]
    fn clip_options_move_onto_the_state() {
        let options = ClipOptions {
            speed: Some(2.0),
            speed_by: Some("Blend".to_owned().into()),
            time_by: Some("Weight".to_owned().into()),
        };
        let compiled = compile(&mut context(), &layer(None, vec![state("A", Some(external("Idle", options)), 51)], vec![])).unwrap();

        assert_eq!(
            compiled.states[0].playback,
            Playback {
                speed: 2.0,
                speed_by: Some(resolved("Blend", AnimatedValueType::Float)),
                time_by: Some(resolved("Weight", AnimatedValueType::Float)),
            }
        );
        assert!(matches!(compiled.states[0].motion, Some(Motion::Clip(Clip::External(_)))));
    }

    fn linear(fields: Vec<BlendTreeField>) -> raw::Motion {
        raw::Motion::BlendTree(raw::BlendTree::Parametric(RawParametricBlendTree {
            tree_type: BlendTreeType::Linear,
            x: "Blend".to_owned().into(),
            y: None,
            fields,
        }))
    }

    #[rstest]
    fn a_blend_tree_resolves_its_axes_and_scales_its_fields() {
        let motion = linear(vec![
            BlendTreeField {
                position: [0.0, 0.0],
                motion: clip(ClipOptions {
                    speed: Some(0.5),
                    ..ClipOptions::default()
                }),
            },
            BlendTreeField {
                position: [1.0, 0.0],
                motion: raw::Motion::BlendTree(raw::BlendTree::Direct(raw::DirectBlendTree {
                    fields: vec![DirectBlendTreeField {
                        weight_by: "Weight".to_owned().into(),
                        motion: external("Idle", ClipOptions::default()),
                    }],
                })),
            },
        ]);
        let compiled = compile(&mut context(), &layer(None, vec![state("A", Some(motion), 51)], vec![])).unwrap();

        let Some(Motion::BlendTree(BlendTree::Parametric(tree))) = &compiled.states[0].motion else {
            panic!("expected a parametric blend tree");
        };
        assert_eq!(tree.x, resolved("Blend", AnimatedValueType::Float));
        assert_eq!(tree.fields[0].speed, 0.5);
        assert_eq!(tree.fields[1].speed, 1.0);
        let Motion::BlendTree(BlendTree::Direct(nested)) = &tree.fields[1].motion else {
            panic!("expected a nested direct blend tree");
        };
        assert_eq!(nested.fields[0].weight_by, resolved("Weight", AnimatedValueType::Float));
        assert_eq!(compiled.states[0].playback, Playback::default());
    }

    #[rstest]
    fn a_field_cannot_follow_a_parameter() {
        let motion = linear(vec![BlendTreeField {
            position: [0.0, 0.0],
            motion: clip(ClipOptions {
                time_by: Some(located("Weight", 55)),
                ..ClipOptions::default()
            }),
        }]);
        let error = compile(&mut context(), &layer(None, vec![state("A", Some(motion), 51)], vec![])).unwrap_err();
        assert_eq!(error, TransformErrorKind::NestedPlayback { option: "time_by" }.at(located_at(55)));
    }

    #[rstest]
    fn a_blend_axis_must_be_a_float() {
        let motion = raw::Motion::BlendTree(raw::BlendTree::Parametric(RawParametricBlendTree {
            tree_type: BlendTreeType::Linear,
            x: located("Emote", 56),
            y: None,
            fields: vec![],
        }));
        let error = compile(&mut context(), &layer(None, vec![state("A", Some(motion), 51)], vec![])).unwrap_err();
        assert_eq!(
            error,
            TransformErrorKind::ParameterTypeMismatch {
                name: "Emote".into(),
                expected: AnimatedValueType::Float,
                found: AnimatedValueType::Int,
            }
            .at(located_at(56))
        );
    }

    fn hat() -> ParameterRef {
        resolved("Hat", AnimatedValueType::Bool)
    }

    fn emote() -> ParameterRef {
        resolved("Emote", AnimatedValueType::Int)
    }

    fn blend() -> ParameterRef {
        resolved("Blend", AnimatedValueType::Float)
    }

    #[rstest]
    #[case::bool_zero(Condition::Zero("Hat".to_owned().into()), AnimatorCondition::IfNot(hat()))]
    #[case::bool_nonzero(Condition::NonZero("Hat".to_owned().into()), AnimatorCondition::If(hat()))]
    #[case::bool_eq_true(Condition::Eq("Hat".to_owned().into(), AnimatedValue::Bool(true)), AnimatorCondition::If(hat()))]
    #[case::bool_eq_false(Condition::Eq("Hat".to_owned().into(), AnimatedValue::Bool(false)), AnimatorCondition::IfNot(hat()))]
    #[case::bool_ne_true(Condition::Ne("Hat".to_owned().into(), AnimatedValue::Bool(true)), AnimatorCondition::IfNot(hat()))]
    #[case::bool_eq_int(Condition::Eq("Hat".to_owned().into(), AnimatedValue::Int(1)), AnimatorCondition::If(hat()))]
    #[case::int_zero(Condition::Zero("Emote".to_owned().into()), AnimatorCondition::Equals(emote(), 0))]
    #[case::int_nonzero(Condition::NonZero("Emote".to_owned().into()), AnimatorCondition::NotEqual(emote(), 0))]
    #[case::int_eq(Condition::Eq("Emote".to_owned().into(), AnimatedValue::Int(3)), AnimatorCondition::Equals(emote(), 3))]
    #[case::int_ne(Condition::Ne("Emote".to_owned().into(), AnimatedValue::Int(3)), AnimatorCondition::NotEqual(emote(), 3))]
    #[case::int_gt(Condition::Gt("Emote".to_owned().into(), AnimatedValue::Int(3)), AnimatorCondition::Greater(emote(), 3.0))]
    #[case::int_lt(Condition::Lt("Emote".to_owned().into(), AnimatedValue::Int(3)), AnimatorCondition::Less(emote(), 3.0))]
    #[case::float_gt(Condition::Gt("Blend".to_owned().into(), AnimatedValue::Float(0.5)), AnimatorCondition::Greater(blend(), 0.5))]
    #[case::float_lt_int(Condition::Lt("Blend".to_owned().into(), AnimatedValue::Int(1)), AnimatorCondition::Less(blend(), 1.0))]
    fn conditions_take_the_comparison_of_the_parameter_type(#[case] written: Condition, #[case] expected: AnimatorCondition) {
        assert_eq!(condition(&context(), &written).unwrap(), expected);
    }

    #[rstest]
    #[case::float_zero(Condition::Zero(located("Blend", 60)), "zero", AnimatedValueType::Float)]
    #[case::float_nonzero(Condition::NonZero(located("Blend", 60)), "nonzero", AnimatedValueType::Float)]
    #[case::float_eq(Condition::Eq(located("Blend", 60), AnimatedValue::Float(0.5)), "eq", AnimatedValueType::Float)]
    #[case::float_ne(Condition::Ne(located("Blend", 60), AnimatedValue::Float(0.5)), "ne", AnimatedValueType::Float)]
    #[case::bool_gt(Condition::Gt(located("Hat", 60), AnimatedValue::Bool(true)), "gt", AnimatedValueType::Bool)]
    #[case::bool_lt(Condition::Lt(located("Hat", 60), AnimatedValue::Bool(true)), "lt", AnimatedValueType::Bool)]
    fn unsupported_comparisons_are_rejected(#[case] written: Condition, #[case] name: &'static str, #[case] value_type: AnimatedValueType) {
        let error = condition(&context(), &written).unwrap_err();
        assert_eq!(
            error,
            TransformErrorKind::UnsupportedCondition {
                condition: name,
                parameter: written.parameter().value.clone(),
                value_type,
            }
            .at(located_at(60))
        );
    }

    #[rstest]
    fn a_condition_value_of_the_wrong_type_is_rejected() {
        let written = Condition::Eq(located("Emote", 61), AnimatedValue::Vector2([0.0, 0.0].into()));
        let error = condition(&context(), &written).unwrap_err();
        assert_eq!(
            error,
            TransformErrorKind::ValueTypeMismatch {
                expected: AnimatedValueType::Int,
                found: AnimatedValueType::Vector2,
            }
            .at(located_at(61))
        );
    }
}
