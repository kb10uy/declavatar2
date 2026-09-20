use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, Value, Variadic};

use crate::{
    core::resolution::Unresolved,
    decl::{
        avatar::Export,
        behavior::Content,
        layer::{BlendLayer, GroupLayer, GroupOption, Layer, PuppetKeyframe, PuppetLayer, SwitchContent, SwitchLayer, SwitchSource},
    },
    lua::{
        content, list,
        location::caller_location,
        node,
        options::{Options, with_children},
        value::StrictBoolean,
    },
};

pub(crate) fn register(lua: &Lua, da: &Table) -> LuaResult<()> {
    da.set("default", lua.create_function(default)?)?;
    da.set("option", lua.create_function(option)?)?;
    da.set("group_layer", lua.create_function(group_layer)?)?;
    da.set("switch_layer", lua.create_function(switch_layer)?)?;
    da.set("keyframe", lua.create_function(keyframe)?)?;
    da.set("puppet_layer", lua.create_function(puppet_layer)?)?;
    da.set("blend_layer", lua.create_function(blend_layer)?)?;
    da.set("gate", lua.create_function(gate)?)?;
    da.set("guard", lua.create_function(guard)?)?;
    Ok(())
}

fn default(lua: &Lua, written: Table) -> LuaResult<node::GroupDefault> {
    Ok(node::GroupDefault(content::content(lua, "da.default", &written)?))
}

fn option(lua: &Lua, (name, written): (String, Table)) -> LuaResult<node::GroupOption> {
    Ok(node::GroupOption(GroupOption {
        name,
        content: content::content(lua, "da.option", &written)?,
        at: caller_location(lua),
    }))
}

/// Child of a group layer, which is either its default state or one of its options.
enum GroupChild {
    Default(Content),
    Option(GroupOption),
}

impl FromLua for GroupChild {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::UserData(userdata) if userdata.is::<node::GroupDefault>() => Ok(Self::Default(node::GroupDefault::from_lua(value.clone(), lua)?.0)),
            Value::UserData(userdata) if userdata.is::<node::GroupOption>() => Ok(Self::Option(node::GroupOption::from_lua(value.clone(), lua)?.0)),
            other => Err(LuaError::runtime(format!(
                "expected `da.default` or `da.option`, got {}",
                node::describe(other)
            ))),
        }
    }
}

fn group_layer(lua: &Lua, (name, arguments): (String, Variadic<Value>)) -> LuaResult<node::Layer> {
    const OWNER: &str = "da.group_layer";

    let (table, children) = with_children(lua, OWNER, arguments)?;
    let mut options = Options::new(OWNER, table);
    let driven_by = options.take::<String>("driven_by")?.map(|parameter| located(lua, parameter));
    let symmetric = options.take::<StrictBoolean>("symmetric")?.map(|value| value.0);
    options.finish()?;

    let mut default = None;
    let mut written_options = Vec::new();
    for child in list::collect::<GroupChild>(lua, OWNER, &children)? {
        match child {
            GroupChild::Default(_) if default.is_some() => {
                return Err(LuaError::runtime(format!("{OWNER}: `{name}` writes `da.default` more than once")));
            }
            GroupChild::Default(content) => default = Some(content),
            GroupChild::Option(option) => written_options.push(option),
        }
    }

    Ok(node::Layer(Layer::Group(GroupLayer {
        name,
        driven_by,
        symmetric,
        default,
        options: written_options,
        at: caller_location(lua),
    })))
}

fn switch_layer(lua: &Lua, (name, table, first, second): (String, Table, Table, Option<Table>)) -> LuaResult<node::Layer> {
    const OWNER: &str = "da.switch_layer";

    let mut options = Options::new(OWNER, Some(table));
    let source = switch_source(lua, OWNER, &mut options)?;
    options.finish()?;

    let content = match second {
        Some(enabled) => SwitchContent::Sides {
            off: content::content(lua, "da.switch_layer: disabled", &first)?,
            on: content::content(lua, "da.switch_layer: enabled", &enabled)?,
        },
        None => {
            let toggle = content::content(lua, "da.switch_layer: toggle", &first)?;
            reject_zeroes(OWNER, &name, &toggle)?;
            SwitchContent::Toggle(toggle)
        }
    };

    Ok(node::Layer(Layer::Switch(SwitchLayer {
        name,
        source,
        content,
        at: caller_location(lua),
    })))
}

fn switch_source(lua: &Lua, owner: &'static str, options: &mut Options) -> LuaResult<Option<SwitchSource>> {
    let driven_by = options.take::<String>("driven_by")?;
    let gate = options.take::<String>("gate")?;

    match (driven_by, gate) {
        (Some(_), Some(_)) => Err(LuaError::runtime(format!(
            "{owner}: a switch layer follows either `driven_by` or `gate`, not both"
        ))),
        (Some(parameter), None) => Ok(Some(SwitchSource::Parameter(located(lua, parameter)))),
        (None, Some(gate)) => Ok(Some(SwitchSource::Gate(located(lua, gate)))),
        (None, None) => Ok(None),
    }
}

/// A toggle list spells out the enabled side, so a value equal to its own zero says nothing.
fn reject_zeroes(owner: &'static str, name: &str, toggle: &Content) -> LuaResult<()> {
    for (_, entry) in toggle.animation.entries() {
        if entry.value.zeroed().as_ref() == Some(&entry.value) {
            return Err(LuaError::runtime(format!(
                "{owner}: the toggle list of `{name}` writes {:?}, which is the value the disabled side already gets; \
                 write both lists to spell out the disabled side",
                entry.value
            )));
        }
    }
    Ok(())
}

fn keyframe(lua: &Lua, (time, targets): (f64, Table)) -> LuaResult<node::Keyframe> {
    Ok(node::Keyframe(PuppetKeyframe {
        time,
        animation: content::animation(lua, "da.keyframe", &targets)?,
    }))
}

fn puppet_layer(lua: &Lua, (name, arguments): (String, Variadic<Value>)) -> LuaResult<node::Layer> {
    const OWNER: &str = "da.puppet_layer";

    let (table, keyframes) = with_children(lua, OWNER, arguments)?;
    let mut options = Options::new(OWNER, table);
    let driven_by = options.take::<String>("driven_by")?.map(|parameter| located(lua, parameter));
    options.finish()?;

    Ok(node::Layer(Layer::Puppet(PuppetLayer {
        name,
        driven_by,
        keyframes: list::collect::<node::Keyframe>(lua, OWNER, &keyframes)?
            .into_iter()
            .map(node::Keyframe::into_inner)
            .collect(),
        at: caller_location(lua),
    })))
}

fn blend_layer(lua: &Lua, (name, children): (String, Table)) -> LuaResult<node::Layer> {
    const OWNER: &str = "da.blend_layer";

    let mut puppets = Vec::new();
    for (offset, child) in list::collect::<node::Layer>(lua, OWNER, &children)?.into_iter().enumerate() {
        match child.0 {
            Layer::Puppet(puppet) => puppets.push(puppet),
            other => {
                return Err(LuaError::runtime(format!(
                    "{OWNER}: entry {} of `{name}` is a {} layer; only a puppet layer has no behaviors and is driven by a float, \
                     so only a puppet layer can be merged",
                    offset + 1,
                    layer_kind(&other),
                )));
            }
        }
    }

    Ok(node::Layer(Layer::Blend(BlendLayer {
        name,
        puppets,
        at: caller_location(lua),
    })))
}

fn gate(lua: &Lua, name: String) -> LuaResult<node::Export> {
    Ok(node::Export(Export::Gate {
        name,
        at: caller_location(lua),
    }))
}

fn guard(lua: &Lua, (gate, parameter): (String, String)) -> LuaResult<node::Export> {
    Ok(node::Export(Export::Guard {
        gate: located(lua, gate),
        parameter: located(lua, parameter),
    }))
}

fn layer_kind(layer: &Layer) -> &'static str {
    match layer {
        Layer::Group(_) => "group",
        Layer::Switch(_) => "switch",
        Layer::Puppet(_) => "puppet",
        Layer::Blend(_) => "blend",
        Layer::Raw(_) => "raw",
    }
}

fn located<T>(lua: &Lua, value: T) -> Unresolved<T> {
    match caller_location(lua) {
        Some(at) => Unresolved::located(value, at),
        None => Unresolved::new(value),
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::{
        core::resolution::SourceLocation,
        decl::behavior::{Behavior, Drive},
        lua::testing::{eval, eval_error},
        unity::{animator::AnimatedTarget, value::AnimatedValue},
    };

    fn layer_of(expression: &str) -> Layer {
        let (lua, value) = eval(expression);
        node::Layer::from_lua(value, &lua).expect("a layer should be built").0
    }

    fn export_of(expression: &str) -> Export {
        let (lua, value) = eval(expression);
        node::Export::from_lua(value, &lua).expect("an export should be built").0
    }

    fn group_of(expression: &str) -> GroupLayer {
        match layer_of(expression) {
            Layer::Group(group) => group,
            other => panic!("expected a group layer, got {other:?}"),
        }
    }

    fn switch_of(expression: &str) -> SwitchLayer {
        match layer_of(expression) {
            Layer::Switch(switch) => switch,
            other => panic!("expected a switch layer, got {other:?}"),
        }
    }

    fn puppet_of(expression: &str) -> PuppetLayer {
        match layer_of(expression) {
            Layer::Puppet(puppet) => puppet,
            other => panic!("expected a puppet layer, got {other:?}"),
        }
    }

    fn shape_names(content: &Content) -> Vec<String> {
        content
            .animation
            .entries()
            .map(|(key, _)| match key {
                AnimatedTarget::Renderer(renderer) => format!("{:?}", renderer.property),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[rstest]
    fn a_group_layer_takes_its_default_and_its_options_in_one_list() {
        let group = group_of(
            "da.group_layer('Expressions', { driven_by = 'Emote' }, {\
             da.default { da.renderer('Face'):shape('eyelid', 0.3) },\
             da.option('smile', { da.renderer('Face'):shape('smile') }),\
             da.option('angry', { da.renderer('Face'):shape('angry') }),\
             })",
        );

        assert_eq!(group.name, "Expressions");
        assert_eq!(group.driven_by, Some(Unresolved::new("Emote".into())));
        assert_eq!(group.symmetric, None);
        assert_eq!(
            group.default.as_ref().map(shape_names),
            Some(vec!["BlendShape { name: \"eyelid\" }".to_string()])
        );
        assert_eq!(group.options.iter().map(|option| option.name.clone()).collect::<Vec<_>>(), ["smile", "angry"]);
    }

    #[rstest]
    fn a_group_layer_needs_no_default() {
        let group = group_of("da.group_layer('Expressions', {}, { da.option('smile', { da.renderer('Face'):shape('smile') }) })");
        assert!(group.default.is_none());
    }

    #[rstest]
    fn a_group_layer_takes_one_default_at_most() {
        let message = eval_error("da.group_layer('Expressions', {}, { da.default {}, da.default {} })");
        assert!(
            message.contains("da.group_layer: `Expressions` writes `da.default` more than once"),
            "{message}"
        );
    }

    #[rstest]
    fn symmetric_is_taken_as_written() {
        assert_eq!(group_of("da.group_layer('a', { symmetric = false }, {})").symmetric, Some(false));
        assert_eq!(group_of("da.group_layer('a', { symmetric = true }, {})").symmetric, Some(true));
    }

    #[rstest]
    fn a_group_child_must_be_a_default_or_an_option() {
        let message = eval_error("da.group_layer('a', {}, { da.renderer('Face'):shape('smile') })");
        assert!(message.contains("expected `da.default` or `da.option`, got target"), "{message}");
    }

    #[rstest]
    fn an_option_mixes_targets_with_drives_and_behaviors() {
        let group = group_of(
            "da.group_layer('a', {}, { da.option('smile', {\
             da.renderer('Face'):shape('smile'),\
             da.drive_bool('Happy', true),\
             da.tracking('animation', { 'mouth' }),\
             }) })",
        );

        let content = &group.options[0].content;
        assert_eq!(content.animation.entries().count(), 1);
        assert_eq!(content.behaviors.len(), 2);
        assert!(matches!(content.behaviors[0], Behavior::Drive(Drive::Parameter { .. })));
        assert!(matches!(content.behaviors[1], Behavior::TrackingControl(_)));
    }

    #[rstest]
    fn the_later_of_two_entries_for_one_target_wins() {
        let group = group_of(
            "da.group_layer('a', {}, { da.option('smile', {\
             da.renderer('Face'):shape('smile', 0.3),\
             da.renderer('Face'):shape('smile', 0.7),\
             }) })",
        );

        let content = &group.options[0].content;
        let values: Vec<_> = content.animation.entries().map(|(_, entry)| entry.value.clone()).collect();
        assert_eq!(values, [AnimatedValue::Float(0.7)]);
    }

    #[rstest]
    fn three_arguments_make_a_switch_layer_a_toggle_list() {
        let switch = switch_of("da.switch_layer('Hat', { driven_by = 'Hat' }, { da.object('Hat'):active() })");

        assert_eq!(switch.source, Some(SwitchSource::Parameter(Unresolved::new("Hat".into()))));
        let SwitchContent::Toggle(toggle) = switch.content else {
            panic!("expected a toggle list");
        };
        assert_eq!(toggle.animation.entries().count(), 1);
    }

    #[rstest]
    fn four_arguments_spell_out_both_sides_of_a_switch_layer() {
        let switch = switch_of(
            "da.switch_layer('Hat', { driven_by = 'Hat' },\
             { da.renderer('Body'):material(0, 'Bare') },\
             { da.renderer('Body'):material(0, 'Dressed') })",
        );

        let SwitchContent::Sides { off, on } = switch.content else {
            panic!("expected both sides");
        };
        assert_eq!(off.animation.entries().count(), 1);
        assert_eq!(on.animation.entries().count(), 1);
    }

    #[rstest]
    fn an_empty_options_table_still_makes_three_arguments_a_toggle_list() {
        let switch = switch_of("da.switch_layer('Hat', {}, { da.object('Hat'):active() })");

        assert_eq!(switch.source, None);
        assert!(matches!(switch.content, SwitchContent::Toggle(_)));
    }

    #[rstest]
    fn an_empty_disabled_list_is_not_confused_with_a_toggle_list() {
        let switch = switch_of("da.switch_layer('Hat', {}, {}, { da.object('Hat'):active() })");

        let SwitchContent::Sides { off, on } = switch.content else {
            panic!("expected both sides");
        };
        assert_eq!(off.animation.entries().count(), 0);
        assert_eq!(on.animation.entries().count(), 1);
    }

    #[rstest]
    fn a_switch_layer_follows_a_gate_instead_of_a_parameter() {
        let switch = switch_of("da.switch_layer('Hat', { gate = 'HatShown' }, { da.object('Hat'):active() })");
        assert_eq!(switch.source, Some(SwitchSource::Gate(Unresolved::new("HatShown".into()))));
    }

    #[rstest]
    fn a_switch_layer_cannot_follow_both_a_parameter_and_a_gate() {
        let message = eval_error("da.switch_layer('Hat', { driven_by = 'Hat', gate = 'HatShown' }, { da.object('Hat'):active() })");
        assert!(message.contains("either `driven_by` or `gate`, not both"), "{message}");
    }

    #[rstest]
    #[case::inactive("da.object('Hat'):active(false)")]
    #[case::zero_shape("da.renderer('Face'):shape('smile', 0)")]
    #[case::zero_property("da.component('Root', 'VRCPhysBone'):property('pull', 0.0)")]
    fn a_toggle_list_refuses_a_value_that_is_already_the_disabled_one(#[case] written: &str) {
        let message = eval_error(&format!("da.switch_layer('Hat', {{}}, {{ {written} }})"));
        assert!(message.contains("which is the value the disabled side already gets"), "{message}");
        assert!(message.contains("write both lists"), "{message}");
    }

    #[rstest]
    fn both_sides_may_write_whatever_they_want() {
        let switch = switch_of("da.switch_layer('Hat', {}, { da.object('Hat'):active(true) }, { da.object('Hat'):active(false) })");
        assert!(matches!(switch.content, SwitchContent::Sides { .. }));
    }

    #[rstest]
    fn a_puppet_layer_holds_keyframes_in_the_order_written() {
        let puppet = puppet_of(
            "da.puppet_layer('Wink', { driven_by = 'WinkAmount' }, {\
             da.keyframe(-1, { da.renderer('Face'):shape('wink_l') }),\
             da.keyframe(0, {}),\
             da.keyframe(1, { da.renderer('Face'):shape('wink_r') }),\
             })",
        );

        assert_eq!(puppet.driven_by, Some(Unresolved::new("WinkAmount".into())));
        assert_eq!(puppet.keyframes.iter().map(|keyframe| keyframe.time).collect::<Vec<_>>(), [-1.0, 0.0, 1.0]);
        assert_eq!(puppet.keyframes[1].animation.entries().count(), 0);
    }

    #[rstest]
    fn a_keyframe_holds_targets_only() {
        let message = eval_error("da.keyframe(0, { da.drive_bool('Happy', true) })");
        assert!(message.contains("da.keyframe: entry 1: expected target, got drive"), "{message}");
    }

    #[rstest]
    fn a_blend_layer_merges_puppet_layers() {
        let layer = layer_of(
            "da.blend_layer('Merged', {\
             da.puppet_layer('Wink', { driven_by = 'A' }, {}),\
             da.puppet_layer('Brow', { driven_by = 'B' }, {}),\
             })",
        );

        let Layer::Blend(blend) = layer else {
            panic!("expected a blend layer");
        };
        assert_eq!(blend.name, "Merged");
        assert_eq!(blend.puppets.iter().map(|puppet| puppet.name.clone()).collect::<Vec<_>>(), ["Wink", "Brow"]);
    }

    #[rstest]
    fn a_blend_layer_takes_puppet_layers_only() {
        let message = eval_error("da.blend_layer('Merged', { da.switch_layer('Hat', {}, { da.object('Hat'):active() }) })");
        assert!(message.contains("da.blend_layer: entry 1 of `Merged` is a switch layer"), "{message}");
        assert!(message.contains("only a puppet layer can be merged"), "{message}");
    }

    #[rstest]
    fn exports_declare_gates_and_guards() {
        assert_eq!(
            export_of("da.gate('HatShown')"),
            Export::Gate {
                name: "HatShown".into(),
                at: Some(SourceLocation {
                    chunk: "test.lua".into(),
                    line: 2,
                }),
            },
        );
        assert_eq!(
            export_of("da.guard('HatShown', 'Hat')"),
            Export::Guard {
                gate: Unresolved::new("HatShown".into()),
                parameter: Unresolved::new("Hat".into()),
            },
        );
    }

    #[rstest]
    fn a_layer_records_where_it_was_written() {
        let (lua, value) = eval("(function()\n  return da.group_layer('a', {}, {})\nend)()");
        let layer = node::Layer::from_lua(value, &lua).expect("a layer should be built").0;

        assert_eq!(
            layer.at(),
            Some(&SourceLocation {
                chunk: "test.lua".into(),
                line: 3,
            })
        );
    }
}
