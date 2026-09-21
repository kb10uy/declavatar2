use mlua::{Error as LuaError, Lua, Result as LuaResult, Table, Value, Variadic};

use crate::{
    decl::controller::Controller,
    lua::{
        api::target::located,
        list,
        location::caller_location,
        node,
        options::{Options, one_of, with_children},
    },
    unity::animator::{MergeMode, PathMode},
    vrchat::playable_layer::PlayableLayer,
};

pub(crate) const PLAYABLE_LAYERS: &[(&str, PlayableLayer)] = &[
    ("base", PlayableLayer::Base),
    ("additive", PlayableLayer::Additive),
    ("gesture", PlayableLayer::Gesture),
    ("action", PlayableLayer::Action),
    ("fx", PlayableLayer::Fx),
    ("sitting", PlayableLayer::Sitting),
    ("tpose", PlayableLayer::TPose),
    ("ikpose", PlayableLayer::IkPose),
];

pub(crate) const MERGE_MODES: &[(&str, MergeMode)] = &[("append", MergeMode::Append), ("replace", MergeMode::Replace)];

pub(crate) const PATH_MODES: &[(&str, PathMode)] = &[("absolute", PathMode::Absolute), ("relative", PathMode::Relative)];

pub(crate) fn register(lua: &Lua, da: &Table) -> LuaResult<()> {
    da.set("controller", lua.create_function(controller)?)?;
    Ok(())
}

fn controller(lua: &Lua, (playable, arguments): (String, Variadic<Value>)) -> LuaResult<node::Controller> {
    const OWNER: &str = "da.controller";

    let playable = one_of(OWNER, "playable layer", &playable, PLAYABLE_LAYERS)?;
    let (table, children) = with_children(lua, OWNER, arguments)?;

    let mut options = Options::new(OWNER, table);
    let mode = options
        .take::<String>("mode")?
        .map(|written| one_of(OWNER, "mode", &written, MERGE_MODES))
        .transpose()?;
    let priority = options.take::<Value>("priority")?.map(|value| priority(OWNER, &value)).transpose()?;
    let path_mode = options
        .take::<String>("path_mode")?
        .map(|written| one_of(OWNER, "path mode", &written, PATH_MODES))
        .transpose()?;
    let mask = options.take::<node::Asset>("mask")?.map(|asset| located(lua, asset.0));
    options.finish()?;

    let layers = list::collect::<node::Layer>(lua, OWNER, &children)?.into_iter().map(Into::into).collect();

    Ok(node::Controller(Controller {
        playable,
        mode,
        priority,
        path_mode,
        mask,
        layers,
        at: caller_location(lua),
    }))
}

fn priority(owner: &'static str, value: &Value) -> LuaResult<i32> {
    match value {
        Value::Integer(written) => {
            i32::try_from(*written).map_err(|_| LuaError::runtime(format!("{owner}: option `priority` is out of range, {written} was written")))
        }
        Value::Number(written) => Err(LuaError::runtime(format!(
            "{owner}: option `priority` must be an integer, but {written} was written"
        ))),
        other => Err(LuaError::runtime(format!(
            "{owner}: option `priority` expected an integer, got {}",
            node::describe(other)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::{
        decl::layer::Layer,
        lua::{EvaluateOptions, runtime::create_runtime},
        unity::external::AssetLocator,
    };

    fn eval(expression: &str) -> Controller {
        let lua = create_runtime(&EvaluateOptions::new()).expect("runtime should be prepared");
        let node: node::Controller = lua
            .load(format!("local da = require 'declavatar'\nreturn {expression}"))
            .call(())
            .expect("expression should build a controller");
        node.0
    }

    fn eval_error(expression: &str) -> String {
        let lua = create_runtime(&EvaluateOptions::new()).expect("runtime should be prepared");
        lua.load(format!("local da = require 'declavatar'\nreturn {expression}"))
            .call::<Value>(())
            .expect_err("expression should fail")
            .to_string()
    }

    #[rstest]
    fn a_bare_controller_leaves_every_option_unwritten() {
        let controller = eval("da.controller('fx', { da.group_layer('Expressions', {}) })");

        assert_eq!(controller.playable, PlayableLayer::Fx);
        assert_eq!(controller.mode, None);
        assert_eq!(controller.priority, None);
        assert_eq!(controller.path_mode, None);
        assert_eq!(controller.mask, None);
        assert_eq!(controller.layers.iter().map(Layer::name).collect::<Vec<_>>(), ["Expressions"]);
        assert_eq!(controller.at.map(|at| at.line), Some(2));
    }

    #[rstest]
    fn every_option_is_taken() {
        let controller =
            eval("da.controller('gesture', { mode = 'replace', priority = -10, path_mode = 'relative', mask = da.asset.path('Assets/Hands.mask') }, {})");

        assert_eq!(controller.playable, PlayableLayer::Gesture);
        assert_eq!(controller.mode, Some(MergeMode::Replace));
        assert_eq!(controller.priority, Some(-10));
        assert_eq!(controller.path_mode, Some(PathMode::Relative));
        let mask = controller.mask.expect("mask should be taken");
        assert_eq!(mask.value, AssetLocator::Path("Assets/Hands.mask".into()));
        assert_eq!(mask.at.map(|at| at.line), Some(2));
        assert!(controller.layers.is_empty());
    }

    #[rstest]
    fn every_playable_layer_is_written_in_lower_case() {
        for (written, playable) in PLAYABLE_LAYERS {
            assert_eq!(eval(&format!("da.controller('{written}', {{}})")).playable, *playable);
        }
    }

    #[rstest]
    fn an_unknown_name_lists_the_accepted_ones() {
        let message = eval_error("da.controller('FX', {})");
        assert!(message.contains("da.controller: playable layer `FX` is not known"), "{message}");
        assert!(
            message.contains("`base`, `additive`, `gesture`, `action`, `fx`, `sitting`, `tpose`, `ikpose`"),
            "{message}"
        );

        let message = eval_error("da.controller('fx', { mode = 'merge' }, {})");
        assert!(message.contains("da.controller: mode `merge` is not known"), "{message}");
        assert!(message.contains("`append`, `replace`"), "{message}");

        let message = eval_error("da.controller('fx', { path_mode = 'root' }, {})");
        assert!(message.contains("da.controller: path mode `root` is not known"), "{message}");
        assert!(message.contains("`absolute`, `relative`"), "{message}");
    }

    #[rstest]
    fn priority_takes_an_integer_only() {
        let message = eval_error("da.controller('fx', { priority = 1.5 }, {})");
        assert!(
            message.contains("da.controller: option `priority` must be an integer, but 1.5 was written"),
            "{message}"
        );

        let message = eval_error("da.controller('fx', { priority = 'high' }, {})");
        assert!(
            message.contains("da.controller: option `priority` expected an integer, got string"),
            "{message}"
        );
    }

    #[rstest]
    fn mask_needs_an_explicit_locator() {
        let message = eval_error("da.controller('fx', { mask = 'Hands' }, {})");
        assert!(message.contains("da.controller: option `mask`"), "{message}");
        assert!(message.contains("expected asset, got string"), "{message}");
    }

    #[rstest]
    fn a_child_must_be_a_layer() {
        let message = eval_error("da.controller('fx', { da.bool('Hat') })");
        assert!(message.contains("da.controller: entry 1: expected layer, got parameter"), "{message}");
    }

    #[rstest]
    fn a_leftover_option_is_rejected() {
        let message = eval_error("da.controller('fx', { layer_priority = 1 }, {})");
        assert!(message.contains("da.controller: unknown option `layer_priority`"), "{message}");
        assert!(message.contains("known options are mode, priority, path_mode, mask"), "{message}");
    }
}
