use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, Value, Variadic};

use crate::{
    core::resolution::Unresolved,
    decl::{
        layer::Layer,
        raw::{
            BlendTree, BlendTreeField, BlendTreeType, ClipOptions, Condition, DirectBlendTree, DirectBlendTreeField, Motion, ParametricBlendTree, RawLayer,
            RawState, RawTransition,
        },
    },
    lua::{
        api::target::{AssetArgument, located},
        content, list,
        location::caller_location,
        node,
        options::{Options, one_of},
        value::animated_value,
    },
    unity::value::AnimatedValue,
};

const CLIP_TYPE: &str = "UnityEngine.AnimationClip";

pub(crate) const PARAMETRIC_TREE_TYPES: &[(&str, BlendTreeType)] = &[
    ("linear", BlendTreeType::Linear),
    ("simple_2d", BlendTreeType::Simple2d),
    ("freeform_2d", BlendTreeType::Freeform2d),
    ("cartesian_2d", BlendTreeType::Cartesian2d),
];

pub(crate) const DIRECT_TREE_TYPE: &str = "direct";

pub(crate) fn register(lua: &Lua, da: &Table) -> LuaResult<()> {
    let raw = lua.create_table()?;
    raw.set("layer", lua.create_function(layer)?)?;
    raw.set("state", lua.create_function(state)?)?;
    raw.set("transition", lua.create_function(transition)?)?;
    raw.set("clip", lua.create_function(clip)?)?;
    raw.set("external", lua.create_function(external)?)?;
    raw.set("blend_tree", lua.create_function(blend_tree)?)?;
    raw.set("field", lua.create_function(field)?)?;
    raw.set("weighted", lua.create_function(weighted)?)?;
    raw.set("cond", condition_table(lua)?)?;

    da.set("raw", raw)?;
    Ok(())
}

/// State together with the transitions written inside it, which the layer flattens.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingState {
    pub state: RawState,
    pub transitions: Vec<PendingTransition>,
}

/// Transition whose `from` is filled in by the state that holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingTransition {
    pub from: Option<Unresolved<String>>,
    pub to: Unresolved<String>,
    pub duration: Option<f64>,
    pub conditions: Vec<Condition>,
}

/// Reference to a state, written either as its name or as the state itself.
struct StateName(String);

impl FromLua for StateName {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::String(name) => Ok(Self(name.to_string_lossy())),
            Value::UserData(userdata) if userdata.is::<node::RawState>() => Ok(Self(node::RawState::from_lua(value.clone(), lua)?.0.state.name)),
            other => Err(LuaError::runtime(format!("expected a state name or a state, got {}", node::describe(other)))),
        }
    }
}

/// Child of a raw layer, which is either one of its states or a transition between two of them.
enum LayerChild {
    State(PendingState),
    Transition(PendingTransition),
}

impl FromLua for LayerChild {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::UserData(userdata) if userdata.is::<node::RawState>() => Ok(Self::State(node::RawState::from_lua(value.clone(), lua)?.0)),
            Value::UserData(userdata) if userdata.is::<node::RawTransition>() => Ok(Self::Transition(node::RawTransition::from_lua(value.clone(), lua)?.0)),
            other => Err(LuaError::runtime(format!(
                "expected `da.raw.state` or `da.raw.transition`, got {}",
                node::describe(other)
            ))),
        }
    }
}

fn layer(lua: &Lua, (name, table, children): (String, Option<Table>, Table)) -> LuaResult<node::Layer> {
    const OWNER: &str = "da.raw.layer";

    let mut options = Options::new(OWNER, table);
    let default_state = options.take::<StateName>("default")?.map(|state| located(lua, state.0));
    options.finish()?;

    let mut states = Vec::new();
    let mut transitions = Vec::new();
    for child in list::collect::<LayerChild>(lua, OWNER, &children)? {
        match child {
            LayerChild::State(pending) => {
                let from = located(lua, pending.state.name.clone());
                states.push(pending.state);
                transitions.extend(pending.transitions.into_iter().map(|written| settle(written, Some(from.clone()))));
            }
            LayerChild::Transition(written) => {
                if written.from.is_none() {
                    return Err(LuaError::runtime(format!(
                        "{OWNER}: a transition written in `{name}` itself needs the state it leaves, \
                         because only one written inside a state can leave it out"
                    )));
                }
                transitions.push(settle(written, None));
            }
        }
    }

    Ok(node::Layer(Layer::Raw(RawLayer {
        name,
        default_state,
        states,
        transitions,
        at: caller_location(lua),
    })))
}

fn settle(written: PendingTransition, holder: Option<Unresolved<String>>) -> RawTransition {
    RawTransition {
        from: written.from.or(holder).expect("a transition has a source by now"),
        to: written.to,
        duration: written.duration,
        conditions: written.conditions,
    }
}

fn state(lua: &Lua, (name, table, outgoing): (String, Option<Table>, Option<Table>)) -> LuaResult<node::RawState> {
    const OWNER: &str = "da.raw.state";

    let mut options = Options::new(OWNER, table);
    let motion = options.take::<node::Motion>("motion")?.map(node::Motion::into_inner);
    let behaviors = match options.take::<Table>("behaviors")? {
        Some(written) => content::behaviors(lua, "da.raw.state: behaviors", &written)?,
        None => Vec::new(),
    };
    options.finish()?;

    let transitions = match outgoing {
        Some(written) => list::collect::<node::RawTransition>(lua, OWNER, &written)?
            .into_iter()
            .map(node::RawTransition::into_inner)
            .collect(),
        None => Vec::new(),
    };

    Ok(node::RawState(PendingState {
        state: RawState {
            name,
            motion,
            behaviors,
            at: caller_location(lua),
        },
        transitions,
    }))
}

fn transition(lua: &Lua, arguments: Variadic<Value>) -> LuaResult<node::RawTransition> {
    const OWNER: &str = "da.raw.transition";

    let mut arguments = arguments.into_iter();
    let (from, to, table, conditions) = match arguments.len() {
        2 => {
            let to = arguments.next().expect("two arguments");
            (None, to, None, arguments.next().expect("two arguments"))
        }
        3 => {
            let first = arguments.next().expect("three arguments");
            let second = arguments.next().expect("three arguments");
            let third = arguments.next().expect("three arguments");
            match second {
                // A table in the second place is the options table, so the first place is the destination.
                Value::Table(options) => (None, first, Some(options), third),
                to => (Some(first), to, None, third),
            }
        }
        4 => {
            let from = arguments.next().expect("four arguments");
            let to = arguments.next().expect("four arguments");
            let Value::Table(options) = arguments.next().expect("four arguments") else {
                return Err(LuaError::runtime(format!("{OWNER}: the third of four arguments is the options table")));
            };
            (Some(from), to, Some(options), arguments.next().expect("four arguments"))
        }
        written => {
            return Err(LuaError::runtime(format!(
                "{OWNER}: expected between two and four arguments, but {written} were written"
            )));
        }
    };

    let mut options = Options::new(OWNER, table);
    let duration = options.take::<f64>("duration")?;
    options.finish()?;

    let Value::Table(conditions) = conditions else {
        return Err(LuaError::runtime(format!(
            "{OWNER}: the last argument is the condition list, but {} was written",
            node::describe(&conditions)
        )));
    };

    Ok(node::RawTransition(PendingTransition {
        from: from
            .map(|written| StateName::from_lua(written, lua))
            .transpose()?
            .map(|state| located(lua, state.0)),
        to: located(lua, StateName::from_lua(to, lua)?.0),
        duration,
        conditions: list::collect::<node::Condition>(lua, OWNER, &conditions)?
            .into_iter()
            .map(node::Condition::into_inner)
            .collect(),
    }))
}

fn clip(lua: &Lua, arguments: Variadic<Value>) -> LuaResult<node::Motion> {
    const OWNER: &str = "da.raw.clip";

    let mut arguments = arguments.into_iter();
    let (table, targets) = match arguments.len() {
        1 => (None, arguments.next().expect("one argument")),
        2 => {
            let Value::Table(options) = arguments.next().expect("two arguments") else {
                return Err(LuaError::runtime(format!("{OWNER}: the first of two arguments is the options table")));
            };
            (Some(options), arguments.next().expect("two arguments"))
        }
        written => {
            return Err(LuaError::runtime(format!("{OWNER}: expected one or two arguments, but {written} were written")));
        }
    };

    let Value::Table(targets) = targets else {
        return Err(LuaError::runtime(format!(
            "{OWNER}: the last argument is the target list, but {} was written",
            node::describe(&targets)
        )));
    };

    let mut options = Options::new(OWNER, table);
    let clip_options = clip_options(lua, &mut options)?;
    options.finish()?;

    Ok(node::Motion(Motion::Clip {
        options: clip_options,
        animation: content::animation(lua, OWNER, &targets)?,
    }))
}

fn external(lua: &Lua, (asset, table): (AssetArgument, Option<Table>)) -> LuaResult<node::Motion> {
    const OWNER: &str = "da.raw.external";

    let mut options = Options::new(OWNER, table);
    let clip_options = clip_options(lua, &mut options)?;
    options.finish()?;

    Ok(node::Motion(Motion::External {
        asset: asset.into_reference(lua, CLIP_TYPE),
        options: clip_options,
    }))
}

fn clip_options(lua: &Lua, options: &mut Options) -> LuaResult<ClipOptions> {
    Ok(ClipOptions {
        speed: options.take::<f64>("speed")?,
        speed_by: options.take::<String>("speed_by")?.map(|parameter| located(lua, parameter)),
        time_by: options.take::<String>("time_by")?.map(|parameter| located(lua, parameter)),
    })
}

fn blend_tree(lua: &Lua, (table, fields): (Table, Table)) -> LuaResult<node::Motion> {
    const OWNER: &str = "da.raw.blend_tree";

    let mut options = Options::new(OWNER, Some(table));
    let written_type = options
        .take::<String>("type")?
        .ok_or_else(|| LuaError::runtime(format!("{OWNER}: option `type` is needed")))?;
    let x = options.take::<String>("x")?;
    let y = options.take::<String>("y")?;
    options.finish()?;

    if written_type == DIRECT_TREE_TYPE {
        return direct_tree(lua, OWNER, (x, y), &fields);
    }

    let tree_type = one_of(OWNER, "type", &written_type, PARAMETRIC_TREE_TYPES)?;
    let x = x.ok_or_else(|| LuaError::runtime(format!("{OWNER}: a `{written_type}` tree blends along `x`")))?;
    match (tree_type.is_two_dimensional(), &y) {
        (true, None) => return Err(LuaError::runtime(format!("{OWNER}: a `{written_type}` tree blends along `y` as well"))),
        (false, Some(_)) => return Err(LuaError::runtime(format!("{OWNER}: a `{written_type}` tree blends along `x` only"))),
        _ => (),
    }

    Ok(node::Motion(Motion::BlendTree(BlendTree::Parametric(ParametricBlendTree {
        tree_type,
        x: located(lua, x),
        y: y.map(|parameter| located(lua, parameter)),
        fields: list::collect::<node::BlendTreeField>(lua, OWNER, &fields)?
            .into_iter()
            .map(node::BlendTreeField::into_inner)
            .collect(),
    }))))
}

fn direct_tree(lua: &Lua, owner: &'static str, axes: (Option<String>, Option<String>), fields: &Table) -> LuaResult<node::Motion> {
    if axes.0.is_some() || axes.1.is_some() {
        return Err(LuaError::runtime(format!(
            "{owner}: a `direct` tree weights each field by its own parameter, so it has no `x` or `y`"
        )));
    }

    Ok(node::Motion(Motion::BlendTree(BlendTree::Direct(DirectBlendTree {
        fields: list::collect::<node::DirectBlendTreeField>(lua, owner, fields)?
            .into_iter()
            .map(node::DirectBlendTreeField::into_inner)
            .collect(),
    }))))
}

fn field(_: &Lua, (position, motion): (Value, node::Motion)) -> LuaResult<node::BlendTreeField> {
    Ok(node::BlendTreeField(BlendTreeField {
        position: position_of("da.raw.field", &position)?,
        motion: motion.into_inner(),
    }))
}

fn weighted(lua: &Lua, (parameter, motion): (String, node::Motion)) -> LuaResult<node::DirectBlendTreeField> {
    Ok(node::DirectBlendTreeField(DirectBlendTreeField {
        weight_by: located(lua, parameter),
        motion: motion.into_inner(),
    }))
}

fn position_of(owner: &'static str, written: &Value) -> LuaResult<[f64; 2]> {
    match written {
        Value::Integer(along) => Ok([*along as f64, 0.0]),
        Value::Number(along) => Ok([*along, 0.0]),
        other => match animated_value::<()>(owner, other)? {
            AnimatedValue::Vector2(position) => Ok([position.x, position.y]),
            _ => Err(LuaError::runtime(format!(
                "{owner}: a position is one number on a single axis, or two on a pair of them"
            ))),
        },
    }
}

fn condition_table(lua: &Lua) -> LuaResult<Table> {
    let cond = lua.create_table()?;
    cond.set(
        "zero",
        lua.create_function(|lua, parameter: String| Ok(node::Condition(Condition::Zero(located(lua, parameter)))))?,
    )?;
    cond.set(
        "nonzero",
        lua.create_function(|lua, parameter: String| Ok(node::Condition(Condition::NonZero(located(lua, parameter)))))?,
    )?;
    cond.set("eq", comparison(lua, "da.raw.cond.eq", Condition::Eq)?)?;
    cond.set("ne", comparison(lua, "da.raw.cond.ne", Condition::Ne)?)?;
    cond.set("gt", comparison(lua, "da.raw.cond.gt", Condition::Gt)?)?;
    cond.set("lt", comparison(lua, "da.raw.cond.lt", Condition::Lt)?)?;
    Ok(cond)
}

fn comparison(lua: &Lua, owner: &'static str, build: fn(Unresolved<String>, AnimatedValue<()>) -> Condition) -> LuaResult<mlua::Function> {
    lua.create_function(move |lua, (parameter, written): (String, Value)| {
        let value = animated_value(owner, &written)?;
        Ok(node::Condition(build(located(lua, parameter), value)))
    })
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::{
        decl::behavior::{Behavior, Drive},
        lua::testing::{eval, eval_error},
    };

    fn raw_layer_of(expression: &str) -> RawLayer {
        let (lua, value) = eval(expression);
        match node::Layer::from_lua(value, &lua).expect("a layer should be built").0 {
            Layer::Raw(raw) => raw,
            other => panic!("expected a raw layer, got {other:?}"),
        }
    }

    fn motion_of(expression: &str) -> Motion {
        let (lua, value) = eval(expression);
        node::Motion::from_lua(value, &lua).expect("a motion should be built").0
    }

    fn condition_of(expression: &str) -> Condition {
        let (lua, value) = eval(expression);
        node::Condition::from_lua(value, &lua).expect("a condition should be built").0
    }

    fn named(name: &str) -> Unresolved<String> {
        Unresolved::new(name.into())
    }

    fn edges(layer: &RawLayer) -> Vec<(String, String)> {
        layer
            .transitions
            .iter()
            .map(|transition| (transition.from.value.clone(), transition.to.value.clone()))
            .collect()
    }

    #[rstest]
    fn a_raw_layer_flattens_the_transitions_written_inside_its_states() {
        let layer = raw_layer_of(
            "da.raw.layer('Gesture', { default = 'idle' }, {\
             da.raw.state('idle', {}, { da.raw.transition('wave', { da.raw.cond.eq('Gesture', 1) }) }),\
             da.raw.state('wave', {}, { da.raw.transition('idle', { da.raw.cond.ne('Gesture', 1) }) }),\
             })",
        );

        assert_eq!(layer.name, "Gesture");
        assert_eq!(layer.default_state, Some(named("idle")));
        assert_eq!(layer.states.iter().map(|state| state.name.clone()).collect::<Vec<_>>(), ["idle", "wave"]);
        assert_eq!(
            edges(&layer),
            [("idle".to_string(), "wave".to_string()), ("wave".to_string(), "idle".to_string())]
        );
    }

    #[rstest]
    fn a_transition_written_in_the_layer_itself_carries_both_ends() {
        let layer = raw_layer_of(
            "da.raw.layer('Gesture', {}, {\
             da.raw.state('idle'),\
             da.raw.state('wave'),\
             da.raw.transition('idle', 'wave', { da.raw.cond.nonzero('Gesture') }),\
             })",
        );

        assert_eq!(edges(&layer), [("idle".to_string(), "wave".to_string())]);
    }

    #[rstest]
    fn a_transition_in_the_layer_itself_needs_the_state_it_leaves() {
        let message = eval_error("da.raw.layer('Gesture', {}, { da.raw.transition('wave', { da.raw.cond.nonzero('Gesture') }) })");
        assert!(message.contains("needs the state it leaves"), "{message}");
    }

    #[rstest]
    fn a_state_reference_is_written_as_a_name_or_as_the_state_itself() {
        let layer = raw_layer_of(
            "(function()\
             local idle = da.raw.state('idle')\
             return da.raw.layer('Gesture', { default = idle }, {\
             idle,\
             da.raw.state('wave', {}, { da.raw.transition(idle, {}) }),\
             })\
             end)()",
        );

        assert_eq!(layer.default_state, Some(named("idle")));
        assert_eq!(edges(&layer), [("wave".to_string(), "idle".to_string())]);
    }

    #[rstest]
    fn a_table_in_the_second_place_of_three_arguments_is_the_options_table() {
        let layer = raw_layer_of(
            "da.raw.layer('Gesture', {}, { da.raw.state('idle', {}, {\
             da.raw.transition('wave', { duration = 0.25 }, { da.raw.cond.nonzero('Gesture') }),\
             }) })",
        );

        assert_eq!(edges(&layer), [("idle".to_string(), "wave".to_string())]);
        assert_eq!(layer.transitions[0].duration, Some(0.25));
    }

    #[rstest]
    fn four_arguments_spell_out_both_ends_and_the_options() {
        let layer = raw_layer_of(
            "da.raw.layer('Gesture', {}, {\
             da.raw.state('idle'),\
             da.raw.transition('idle', 'wave', { duration = 0.1 }, {}),\
             })",
        );

        assert_eq!(edges(&layer), [("idle".to_string(), "wave".to_string())]);
        assert_eq!(layer.transitions[0].duration, Some(0.1));
    }

    #[rstest]
    fn a_state_holds_a_motion_and_behaviors() {
        let layer = raw_layer_of(
            "da.raw.layer('Gesture', {}, { da.raw.state('wave', {\
             motion = da.raw.clip { da.renderer('Face'):shape('smile') },\
             behaviors = { da.drive_bool('Waving', true) },\
             }) })",
        );

        let state = &layer.states[0];
        assert!(matches!(state.motion, Some(Motion::Clip { .. })));
        assert_eq!(state.behaviors.len(), 1);
        assert!(matches!(state.behaviors[0], Behavior::Drive(Drive::Parameter { .. })));
    }

    #[rstest]
    fn a_behavior_list_holds_no_targets() {
        let message = eval_error("da.raw.state('wave', { behaviors = { da.renderer('Face'):shape('smile') } })");
        assert!(
            message.contains("da.raw.state: behaviors: entry 1: expected drive or behavior, got target"),
            "{message}"
        );
    }

    #[rstest]
    fn a_clip_takes_its_options_before_its_targets() {
        let Motion::Clip { options, animation } = motion_of("da.raw.clip({ speed = 2, time_by = 'Blend' }, { da.renderer('Face'):shape('smile') })") else {
            panic!("expected a clip");
        };

        assert_eq!(options.speed, Some(2.0));
        assert_eq!(options.time_by, Some(named("Blend")));
        assert_eq!(options.speed_by, None);
        assert_eq!(animation.entries().count(), 1);
    }

    #[rstest]
    fn a_clip_without_options_takes_only_its_targets() {
        let Motion::Clip { options, animation } = motion_of("da.raw.clip { da.renderer('Face'):shape('smile') }") else {
            panic!("expected a clip");
        };

        assert_eq!(options, ClipOptions::default());
        assert_eq!(animation.entries().count(), 1);
    }

    #[rstest]
    fn an_external_clip_names_an_asset() {
        let Motion::External { asset, options } = motion_of("da.raw.external('Wave', { speed_by = 'Rate' })") else {
            panic!("expected an external clip");
        };

        assert_eq!(
            asset,
            Unresolved::new(crate::unity::external::AssetLocator::Named {
                asset_type: CLIP_TYPE.into(),
                name: "Wave".into(),
            })
        );
        assert_eq!(options.speed_by, Some(named("Rate")));
    }

    #[rstest]
    #[case::linear("linear", BlendTreeType::Linear)]
    #[case::simple_2d("simple_2d", BlendTreeType::Simple2d)]
    #[case::freeform_2d("freeform_2d", BlendTreeType::Freeform2d)]
    #[case::cartesian_2d("cartesian_2d", BlendTreeType::Cartesian2d)]
    fn a_parametric_tree_blends_along_its_axes(#[case] written: &str, #[case] expected: BlendTreeType) {
        let axes = if expected.is_two_dimensional() { "x = 'X', y = 'Y'" } else { "x = 'X'" };
        let Motion::BlendTree(BlendTree::Parametric(tree)) = motion_of(&format!(
            "da.raw.blend_tree({{ type = '{written}', {axes} }}, {{ da.raw.field(0, da.raw.clip {{}}) }})"
        )) else {
            panic!("expected a parametric tree");
        };

        assert_eq!(tree.tree_type, expected);
        assert_eq!(tree.x, named("X"));
        assert_eq!(tree.y.is_some(), expected.is_two_dimensional());
        assert_eq!(tree.fields.len(), 1);
    }

    #[rstest]
    fn a_linear_tree_takes_a_position_as_one_number() {
        let Motion::BlendTree(BlendTree::Parametric(tree)) = motion_of(
            "da.raw.blend_tree({ type = 'linear', x = 'Blend' }, {\
             da.raw.field(-1, da.raw.clip {}),\
             da.raw.field(1, da.raw.clip {}),\
             })",
        ) else {
            panic!("expected a parametric tree");
        };

        assert_eq!(tree.fields.iter().map(|field| field.position).collect::<Vec<_>>(), [[-1.0, 0.0], [1.0, 0.0]]);
    }

    #[rstest]
    fn a_two_dimensional_tree_takes_a_position_as_a_pair() {
        let Motion::BlendTree(BlendTree::Parametric(tree)) = motion_of(
            "da.raw.blend_tree({ type = 'cartesian_2d', x = 'X', y = 'Y' }, {\
             da.raw.field(da.vec2(-1, 1), da.raw.clip {}),\
             da.raw.field({ 1, -1 }, da.raw.clip {}),\
             })",
        ) else {
            panic!("expected a parametric tree");
        };

        assert_eq!(tree.fields.iter().map(|field| field.position).collect::<Vec<_>>(), [[-1.0, 1.0], [1.0, -1.0]]);
    }

    #[rstest]
    fn a_direct_tree_weights_each_field_by_its_own_parameter() {
        let Motion::BlendTree(BlendTree::Direct(tree)) = motion_of(
            "da.raw.blend_tree({ type = 'direct' }, {\
             da.raw.weighted('WeightA', da.raw.clip {}),\
             da.raw.weighted('WeightB', da.raw.clip {}),\
             })",
        ) else {
            panic!("expected a direct tree");
        };

        assert_eq!(
            tree.fields.iter().map(|field| field.weight_by.clone()).collect::<Vec<_>>(),
            [named("WeightA"), named("WeightB")],
        );
    }

    #[rstest]
    fn a_direct_tree_has_no_axes() {
        let message = eval_error("da.raw.blend_tree({ type = 'direct', x = 'X' }, {})");
        assert!(message.contains("so it has no `x` or `y`"), "{message}");
    }

    #[rstest]
    fn a_parametric_tree_needs_the_axes_its_type_blends_along() {
        let message = eval_error("da.raw.blend_tree({ type = 'linear' }, {})");
        assert!(message.contains("a `linear` tree blends along `x`"), "{message}");

        let message = eval_error("da.raw.blend_tree({ type = 'cartesian_2d', x = 'X' }, {})");
        assert!(message.contains("blends along `y` as well"), "{message}");

        let message = eval_error("da.raw.blend_tree({ type = 'linear', x = 'X', y = 'Y' }, {})");
        assert!(message.contains("blends along `x` only"), "{message}");
    }

    #[rstest]
    fn a_direct_tree_takes_weighted_fields_only() {
        let message = eval_error("da.raw.blend_tree({ type = 'direct' }, { da.raw.field(0, da.raw.clip {}) })");
        assert!(message.contains("expected weighted field, got field"), "{message}");

        let message = eval_error("da.raw.blend_tree({ type = 'linear', x = 'X' }, { da.raw.weighted('W', da.raw.clip {}) })");
        assert!(message.contains("expected field, got weighted field"), "{message}");
    }

    #[rstest]
    fn a_blend_tree_names_its_type() {
        let message = eval_error("da.raw.blend_tree({ x = 'X' }, {})");
        assert!(message.contains("option `type` is needed"), "{message}");

        let message = eval_error("da.raw.blend_tree({ type = 'polar', x = 'X' }, {})");
        assert!(message.contains("type `polar` is not known"), "{message}");
    }

    #[rstest]
    #[case::zero("da.raw.cond.zero('Emote')", Condition::Zero(Unresolved::new("Emote".into())))]
    #[case::nonzero("da.raw.cond.nonzero('Emote')", Condition::NonZero(Unresolved::new("Emote".into())))]
    #[case::eq("da.raw.cond.eq('Emote', 3)", Condition::Eq(Unresolved::new("Emote".into()), AnimatedValue::Int(3)))]
    #[case::ne("da.raw.cond.ne('Emote', 3)", Condition::Ne(Unresolved::new("Emote".into()), AnimatedValue::Int(3)))]
    #[case::gt("da.raw.cond.gt('Blend', 0.5)", Condition::Gt(Unresolved::new("Blend".into()), AnimatedValue::Float(0.5)))]
    #[case::lt("da.raw.cond.lt('Blend', 0.5)", Condition::Lt(Unresolved::new("Blend".into()), AnimatedValue::Float(0.5)))]
    fn conditions_keep_the_parameter_and_the_value_written(#[case] expression: &str, #[case] expected: Condition) {
        assert_eq!(condition_of(expression), expected);
    }

    #[rstest]
    fn a_transition_takes_between_two_and_four_arguments() {
        let message = eval_error("da.raw.transition('wave')");
        assert!(message.contains("expected between two and four arguments, but 1 were written"), "{message}");
    }

    #[rstest]
    fn a_layer_child_must_be_a_state_or_a_transition() {
        let message = eval_error("da.raw.layer('Gesture', {}, { da.renderer('Face'):shape('smile') })");
        assert!(message.contains("expected `da.raw.state` or `da.raw.transition`, got target"), "{message}");
    }
}
