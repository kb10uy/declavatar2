use std::collections::BTreeMap;

use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, Value, Variadic};

use crate::{
    core::{phase::Declared, resolution::Unresolved, value_set::ValueSet},
    decl::{
        behavior::Animation,
        layer::Layer,
        raw::{
            BlendTree, BlendTreeField, BlendTreeType, ClipOptions, Condition, DirectBlendTree, DirectBlendTreeField, Motion, ParametricBlendTree, RawMachine,
            RawState, RawTransition, TransitionSource, TransitionTarget,
        },
    },
    lua::{
        api::target::{AssetArgument, located},
        content, list,
        location::caller_location,
        node,
        options::{Options, one_of, with_children},
        value::{StrictBoolean, animated_value},
    },
    transform::animation::describe,
    unity::{
        animation::{ClipAttributes, Curve, Interpolation, KeyedAnimation, KeyedAnimationEntry, Keyframe},
        animator::AnimatedTarget,
        external::AssetLocator,
        value::AnimatedValue,
    },
};

const CLIP_TYPE: &str = "UnityEngine.AnimationClip";

pub(crate) const PARAMETRIC_TREE_TYPES: &[(&str, BlendTreeType)] = &[
    ("linear", BlendTreeType::Linear),
    ("simple_2d", BlendTreeType::Simple2d),
    ("freeform_2d", BlendTreeType::Freeform2d),
    ("cartesian_2d", BlendTreeType::Cartesian2d),
];

pub(crate) const DIRECT_TREE_TYPE: &str = "direct";

pub(crate) const INTERPOLATIONS: &[(&str, Interpolation)] = &[("constant", Interpolation::Constant), ("linear", Interpolation::Linear)];

pub(crate) fn register(lua: &Lua, da: &Table) -> LuaResult<()> {
    let raw = lua.create_table()?;
    raw.set("layer", lua.create_function(layer)?)?;
    raw.set("machine", lua.create_function(machine)?)?;
    raw.set("state", lua.create_function(state)?)?;
    raw.set("transition", lua.create_function(transition)?)?;
    raw.set("entry", node::RawEntry(()))?;
    raw.set("exit", node::RawExit(()))?;
    raw.set("clip", lua.create_function(clip)?)?;
    raw.set("keyed_clip", lua.create_function(keyed_clip)?)?;
    raw.set("keyframe", lua.create_function(keyframe)?)?;
    raw.set("bezier", lua.create_function(bezier)?)?;
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
    pub from: Option<TransitionSource>,
    pub to: TransitionTarget,
    pub duration: Option<f64>,
    pub conditions: Vec<Condition>,
}

/// Keyframe of a keyed clip, which the clip splits into one curve per target.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingKeyframe {
    pub time: f64,

    /// How every target written here arrives from its previous keyframe, if it has one.
    pub interpolation: Option<Interpolation>,

    pub animation: Animation,
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

/// Reference to a node of a machine, written as its name or as the state or nested machine itself.
struct NodeName(String);

impl FromLua for NodeName {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::String(name) => Ok(Self(name.to_string_lossy())),
            Value::UserData(userdata) if userdata.is::<node::RawState>() => Ok(Self(node::RawState::from_lua(value.clone(), lua)?.0.state.name)),
            Value::UserData(userdata) if userdata.is::<node::RawMachine>() => Ok(Self(node::RawMachine::from_lua(value.clone(), lua)?.0.name)),
            other => Err(LuaError::runtime(format!(
                "expected the name of a state or a state machine, a state or a state machine, got {}",
                node::describe(other)
            ))),
        }
    }
}

fn source_of(lua: &Lua, value: Value) -> LuaResult<TransitionSource> {
    match &value {
        Value::UserData(userdata) if userdata.is::<node::RawEntry>() => Ok(TransitionSource::Entry),
        Value::UserData(userdata) if userdata.is::<node::RawExit>() => Err(LuaError::runtime(
            "`da.raw.exit` is where a transition leads, so it cannot be the place a transition leaves",
        )),
        _ => Ok(TransitionSource::Node(located(lua, NodeName::from_lua(value, lua)?.0))),
    }
}

fn target_of(lua: &Lua, value: Value) -> LuaResult<TransitionTarget> {
    match &value {
        Value::UserData(userdata) if userdata.is::<node::RawExit>() => Ok(TransitionTarget::Exit),
        Value::UserData(userdata) if userdata.is::<node::RawEntry>() => Err(LuaError::runtime(
            "`da.raw.entry` is where a transition leaves, so it cannot be the place a transition leads",
        )),
        _ => Ok(TransitionTarget::Node(located(lua, NodeName::from_lua(value, lua)?.0))),
    }
}

/// Child of a state machine: one of its states, a machine nested in it, or a transition between its nodes.
enum MachineChild {
    State(PendingState),
    Machine(RawMachine),
    Transition(PendingTransition),
}

impl FromLua for MachineChild {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::UserData(userdata) if userdata.is::<node::RawState>() => Ok(Self::State(node::RawState::from_lua(value.clone(), lua)?.0)),
            Value::UserData(userdata) if userdata.is::<node::RawMachine>() => Ok(Self::Machine(node::RawMachine::from_lua(value.clone(), lua)?.0)),
            Value::UserData(userdata) if userdata.is::<node::RawTransition>() => Ok(Self::Transition(node::RawTransition::from_lua(value.clone(), lua)?.0)),
            other => Err(LuaError::runtime(format!(
                "expected `da.raw.state`, `da.raw.machine` or `da.raw.transition`, got {}",
                node::describe(other)
            ))),
        }
    }
}

fn layer(lua: &Lua, (name, arguments): (String, Variadic<Value>)) -> LuaResult<node::Layer> {
    Ok(node::Layer(Layer::Raw(machine_of(lua, "da.raw.layer", name, arguments)?)))
}

fn machine(lua: &Lua, (name, arguments): (String, Variadic<Value>)) -> LuaResult<node::RawMachine> {
    Ok(node::RawMachine(machine_of(lua, "da.raw.machine", name, arguments)?))
}

fn machine_of(lua: &Lua, owner: &'static str, name: String, arguments: Variadic<Value>) -> LuaResult<RawMachine> {
    let (table, children) = with_children(lua, owner, arguments)?;
    let mut options = Options::new(owner, table);
    let default_state = options.take::<StateName>("default")?.map(|state| located(lua, state.0));
    options.finish()?;

    let mut states = Vec::new();
    let mut machines = Vec::new();
    let mut transitions = Vec::new();
    for child in list::collect::<MachineChild>(lua, owner, &children)? {
        match child {
            MachineChild::State(pending) => {
                let from = TransitionSource::Node(located(lua, pending.state.name.clone()));
                states.push(pending.state);
                transitions.extend(pending.transitions.into_iter().map(|written| settle(written, Some(from.clone()))));
            }
            MachineChild::Machine(nested) => machines.push(nested),
            MachineChild::Transition(written) => {
                if written.from.is_none() {
                    return Err(LuaError::runtime(format!(
                        "{owner}: a transition written in `{name}` itself needs the place it leaves, \
                         because only one written inside a state can leave it out"
                    )));
                }
                transitions.push(settle(written, None));
            }
        }
    }

    Ok(RawMachine {
        name,
        default_state,
        states,
        machines,
        transitions,
        at: caller_location(lua),
    })
}

fn settle(written: PendingTransition, holder: Option<TransitionSource>) -> RawTransition {
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

    let from = from.map(|written| source_of(lua, written)).transpose()?;
    let to = target_of(lua, to)?;
    if from == Some(TransitionSource::Entry) {
        if duration.is_some() {
            return Err(LuaError::runtime(format!(
                "{OWNER}: a transition leaving `da.raw.entry` is taken at once, so it has no `duration`"
            )));
        }
        if to == TransitionTarget::Exit {
            return Err(LuaError::runtime(format!(
                "{OWNER}: a transition leaving `da.raw.entry` leads to a state or a state machine, not to `da.raw.exit`"
            )));
        }
    }

    Ok(node::RawTransition(PendingTransition {
        from,
        to,
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

fn keyed_clip(lua: &Lua, arguments: Variadic<Value>) -> LuaResult<node::Motion> {
    const OWNER: &str = "da.raw.keyed_clip";

    let mut arguments = arguments.into_iter();
    let (table, keyframes) = match arguments.len() {
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

    let Value::Table(keyframes) = keyframes else {
        return Err(LuaError::runtime(format!(
            "{OWNER}: the last argument is the keyframe list, but {} was written",
            node::describe(&keyframes)
        )));
    };

    let mut options = Options::new(OWNER, table);
    let clip_options = clip_options(lua, &mut options)?;
    let defaults = ClipAttributes::default();
    let attributes = ClipAttributes {
        length: options.take::<f64>("length")?.unwrap_or(defaults.length),
        loop_time: options.take::<StrictBoolean>("loop_time")?.map_or(defaults.loop_time, |value| value.0),
        loop_blend: options.take::<StrictBoolean>("loop_blend")?.map_or(defaults.loop_blend, |value| value.0),
        cycle_offset: options.take::<f64>("cycle_offset")?.unwrap_or(defaults.cycle_offset),
    };
    options.finish()?;
    if !(attributes.length.is_finite() && attributes.length > 0.0) {
        return Err(LuaError::runtime(format!(
            "{OWNER}: a clip lasts a positive number of seconds, but `length` is {}",
            attributes.length
        )));
    }

    let mut keyframes: Vec<_> = list::collect::<node::ClipKeyframe>(lua, OWNER, &keyframes)?
        .into_iter()
        .map(node::ClipKeyframe::into_inner)
        .collect();
    keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));

    let mut curves: BTreeMap<AnimatedTarget<Declared>, Curve<Unresolved<AssetLocator>>> = BTreeMap::new();
    for written in keyframes {
        for (target, entry) in written.animation.entries() {
            let keyframe = Keyframe {
                time: written.time,
                value: entry.value.clone(),
            };
            match curves.get_mut(target) {
                Some(curve) => {
                    let interpolation = written.interpolation.unwrap_or_else(|| natural_interpolation(&keyframe.value));
                    curve.rest.push((interpolation, keyframe));
                }
                None => {
                    curves.insert(entry.key.clone(), Curve::single(keyframe));
                }
            }
        }
    }

    let mut entries = ValueSet::new();
    for (key, curve) in curves {
        curve
            .validate()
            .map_err(|error| LuaError::runtime(format!("{OWNER}: the curve of {} is invalid: {error}", describe(&key))))?;
        entries.insert(KeyedAnimationEntry { key, curve });
    }

    Ok(node::Motion(Motion::Keyed {
        options: clip_options,
        animation: KeyedAnimation { attributes, curves: entries },
    }))
}

/// Linear where the value can be interpolated, and a step where it cannot.
fn natural_interpolation(value: &AnimatedValue<Unresolved<AssetLocator>>) -> Interpolation {
    if value.value_type().is_interpolable() {
        Interpolation::Linear
    } else {
        Interpolation::Constant
    }
}

fn keyframe(lua: &Lua, (time, arguments): (f64, Variadic<Value>)) -> LuaResult<node::ClipKeyframe> {
    const OWNER: &str = "da.raw.keyframe";

    if !(0.0..=1.0).contains(&time) {
        return Err(LuaError::runtime(format!(
            "{OWNER}: a keyframe sits at normalized time between 0 and 1, but {time} was written"
        )));
    }

    let (table, targets) = with_children(lua, OWNER, arguments)?;
    let mut options = Options::new(OWNER, table);
    let interpolation = options.take::<InterpolationArgument>("interpolation")?.map(|written| written.0);
    options.finish()?;

    Ok(node::ClipKeyframe(PendingKeyframe {
        time,
        interpolation,
        animation: content::animation(lua, OWNER, &targets)?,
    }))
}

fn bezier(_: &Lua, (x1, y1, x2, y2): (f64, f64, f64, f64)) -> LuaResult<node::Interpolation> {
    let interpolation = Interpolation::Bezier { x1, y1, x2, y2 };
    interpolation.validate().map_err(|error| LuaError::runtime(format!("da.raw.bezier: {error}")))?;
    Ok(node::Interpolation(interpolation))
}

/// Interpolation written as its name or as `da.raw.bezier`.
struct InterpolationArgument(Interpolation);

impl FromLua for InterpolationArgument {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::String(name) => Ok(Self(one_of("da.raw.keyframe", "interpolation", &name.to_string_lossy(), INTERPOLATIONS)?)),
            Value::UserData(userdata) if userdata.is::<node::Interpolation>() => Ok(Self(node::Interpolation::from_lua(value.clone(), lua)?.0)),
            other => Err(LuaError::runtime(format!(
                "expected an interpolation name or `da.raw.bezier`, got {}",
                node::describe(other)
            ))),
        }
    }
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

    fn raw_layer_of(expression: &str) -> RawMachine {
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

    fn edges(machine: &RawMachine) -> Vec<(String, String)> {
        let source = |source: &TransitionSource| match source {
            TransitionSource::Entry => "<entry>".to_owned(),
            TransitionSource::Node(name) => name.value.clone(),
        };
        let target = |target: &TransitionTarget| match target {
            TransitionTarget::Exit => "<exit>".to_owned(),
            TransitionTarget::Node(name) => name.value.clone(),
        };
        machine
            .transitions
            .iter()
            .map(|transition| (source(&transition.from), target(&transition.to)))
            .collect()
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        expected.iter().map(|(from, to)| ((*from).to_owned(), (*to).to_owned())).collect()
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
        assert!(message.contains("needs the place it leaves"), "{message}");
    }

    #[rstest]
    fn a_machine_nests_states_machines_and_transitions() {
        let layer = raw_layer_of(
            "da.raw.layer('Emote', {\
             da.raw.state('Idle'),\
             da.raw.machine('Dance', { default = 'Step' }, {\
             da.raw.state('Intro'),\
             da.raw.state('Step', {}, { da.raw.transition(da.raw.exit, { da.raw.cond.zero('Emote') }) }),\
             da.raw.machine('Finale', { da.raw.state('Bow') }),\
             da.raw.transition(da.raw.entry, 'Finale', { da.raw.cond.eq('Emote', 2) }),\
             }),\
             da.raw.transition('Idle', 'Dance', { da.raw.cond.nonzero('Emote') }),\
             da.raw.transition('Dance', 'Idle', {}),\
             })",
        );

        assert_eq!(layer.machines.len(), 1);
        let dance = &layer.machines[0];
        assert_eq!(dance.name, "Dance");
        assert_eq!(dance.default_state, Some(named("Step")));
        assert_eq!(dance.states.iter().map(|state| state.name.clone()).collect::<Vec<_>>(), ["Intro", "Step"]);
        assert_eq!(dance.machines.iter().map(|machine| machine.name.clone()).collect::<Vec<_>>(), ["Finale"]);
        assert_eq!(edges(dance), pairs(&[("Step", "<exit>"), ("<entry>", "Finale")]));
        assert_eq!(edges(&layer), pairs(&[("Idle", "Dance"), ("Dance", "Idle")]));
    }

    #[rstest]
    fn a_machine_reference_is_written_as_a_name_or_as_the_machine_itself() {
        let layer = raw_layer_of(
            "(function()\
             local dance = da.raw.machine('Dance', { da.raw.state('Step') })\
             return da.raw.layer('Emote', {\
             da.raw.state('Idle', {}, { da.raw.transition(dance, {}) }),\
             dance,\
             da.raw.transition(dance, 'Idle', {}),\
             })\
             end)()",
        );

        assert_eq!(edges(&layer), pairs(&[("Idle", "Dance"), ("Dance", "Idle")]));
    }

    #[rstest]
    #[case::exit_as_source("da.raw.transition(da.raw.exit, 'Idle', {})", "`da.raw.exit` is where a transition leads")]
    #[case::entry_as_target("da.raw.transition(da.raw.entry, {})", "`da.raw.entry` is where a transition leaves")]
    #[case::timed_entry("da.raw.transition(da.raw.entry, 'Idle', { duration = 0.5 }, {})", "is taken at once, so it has no `duration`")]
    #[case::entry_to_exit("da.raw.transition(da.raw.entry, da.raw.exit, {})", "not to `da.raw.exit`")]
    #[case::machine_as_default(
        "da.raw.layer('Emote', { default = da.raw.machine('Dance', {}) }, {})",
        "expected a state name or a state, got state machine"
    )]
    #[case::bad_reference(
        "da.raw.transition(da.raw.clip {}, {})",
        "expected the name of a state or a state machine, a state or a state machine, got motion"
    )]
    fn entry_and_exit_stay_on_their_side_of_a_transition(#[case] expression: &str, #[case] expected: &str) {
        let message = eval_error(expression);
        assert!(message.contains(expected), "{message}");
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

    fn keyed_of(expression: &str) -> (ClipOptions, KeyedAnimation<Declared>) {
        let Motion::Keyed { options, animation } = motion_of(expression) else {
            panic!("expected a keyed clip");
        };
        (options, animation)
    }

    fn segments(animation: &KeyedAnimation<Declared>) -> Vec<(f64, Vec<(Interpolation, f64)>)> {
        animation
            .curves
            .entries()
            .map(|(_, entry)| {
                let rest = entry
                    .curve
                    .rest
                    .iter()
                    .map(|(interpolation, keyframe)| (*interpolation, keyframe.time))
                    .collect();
                (entry.curve.first.time, rest)
            })
            .collect()
    }

    #[rstest]
    fn a_keyed_clip_splits_its_keyframes_into_one_curve_per_target() {
        let (options, animation) = keyed_of(
            "da.raw.keyed_clip({ speed = 2, length = 3, loop_time = true, loop_blend = true, cycle_offset = 0.5 }, {\
             da.raw.keyframe(1, { interpolation = 'constant' }, { da.renderer('Face'):shape('smile', 1) }),\
             da.raw.keyframe(0, { da.renderer('Face'):shape('smile', 0), da.object('Hat'):active(false) }),\
             da.raw.keyframe(0.5, { da.object('Hat'):active(true) }),\
             })",
        );

        assert_eq!(options.speed, Some(2.0));
        assert_eq!(
            animation.attributes,
            ClipAttributes {
                length: 3.0,
                loop_time: true,
                loop_blend: true,
                cycle_offset: 0.5,
            }
        );
        assert_eq!(
            segments(&animation),
            vec![(0.0, vec![(Interpolation::Constant, 0.5)]), (0.0, vec![(Interpolation::Constant, 1.0)]),]
        );
    }

    #[rstest]
    fn a_keyed_clip_bends_a_segment_where_the_value_allows_it() {
        let (_, animation) = keyed_of(
            "da.raw.keyed_clip {\
             da.raw.keyframe(0, { da.renderer('Face'):shape('smile', 0) }),\
             da.raw.keyframe(0.5, { interpolation = da.raw.bezier(0.1, 0, 0.9, 1) }, { da.renderer('Face'):shape('smile', 0.5) }),\
             da.raw.keyframe(1, { da.renderer('Face'):shape('smile', 1) }),\
             }",
        );

        assert_eq!(animation.attributes, ClipAttributes::default());
        assert_eq!(
            segments(&animation),
            vec![(
                0.0,
                vec![
                    (
                        Interpolation::Bezier {
                            x1: 0.1,
                            y1: 0.0,
                            x2: 0.9,
                            y2: 1.0
                        },
                        0.5
                    ),
                    (Interpolation::Linear, 1.0),
                ]
            )]
        );
    }

    #[rstest]
    #[case::time_out_of_range("da.raw.keyframe(1.5, {})", "normalized time between 0 and 1, but 1.5 was written")]
    #[case::control_point("da.raw.bezier(1.5, 0, 0.5, 1)", "bezier control point x 1.5 is out of range")]
    #[case::unknown_interpolation("da.raw.keyframe(0, { interpolation = 'cubic' }, {})", "interpolation `cubic` is not known")]
    #[case::length("da.raw.keyed_clip({ length = 0 }, {})", "`length` is 0")]
    #[case::linear_bool(
        "da.raw.keyed_clip { da.raw.keyframe(0, { da.object('Hat'):active(false) }), da.raw.keyframe(1, { interpolation = 'linear' }, { da.object('Hat'):active() }) }",
        "the curve of `Hat` active is invalid: Linear interpolation cannot be applied to Bool values"
    )]
    #[case::same_time(
        "da.raw.keyed_clip { da.raw.keyframe(0.5, { da.object('Hat'):active(false) }), da.raw.keyframe(0.5, { da.object('Hat'):active() }) }",
        "does not increase from previous 0.5"
    )]
    #[case::mixed_types(
        "da.raw.keyed_clip { da.raw.keyframe(0, { da.renderer('Face'):property('_X', 1) }), da.raw.keyframe(1, { da.renderer('Face'):property('_X', 0.5) }) }",
        "expected Int value but found Float"
    )]
    #[case::puppet_keyframe("da.raw.keyed_clip { da.keyframe(0, {}) }", "expected clip keyframe, got keyframe")]
    fn a_bad_keyed_clip_is_rejected_where_it_is_written(#[case] expression: &str, #[case] expected: &str) {
        let message = eval_error(expression);
        assert!(message.contains(expected), "{message}");
        assert!(message.contains("test.lua:2:"), "{message}");
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
        assert!(
            message.contains("expected `da.raw.state`, `da.raw.machine` or `da.raw.transition`, got target"),
            "{message}"
        );
    }
}
