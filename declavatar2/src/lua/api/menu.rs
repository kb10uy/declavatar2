use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, Value};

use crate::{
    decl::menu::{Axis, AxisTarget, FourAxes, MenuItem, TwoAxes},
    lua::{api::target::located, list, node, options::Options},
};

pub(crate) fn register(lua: &Lua, da: &Table) -> LuaResult<()> {
    da.set("submenu", lua.create_function(submenu)?)?;
    da.set("toggle", lua.create_function(toggle)?)?;
    da.set("button", lua.create_function(button)?)?;
    da.set("radial", lua.create_function(radial)?)?;
    da.set("two_axis", lua.create_function(two_axis)?)?;
    da.set("four_axis", lua.create_function(four_axis)?)?;
    da.set("axis", lua.create_function(axis)?)?;
    Ok(())
}

/// What an axis of a puppet item moves, written as a parameter name, a puppet drive or `da.axis`.
struct AxisArgument(Axis);

impl FromLua for AxisArgument {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::String(parameter) => Ok(Self(Axis::bare(AxisTarget::Parameter(located(lua, parameter.to_string_lossy()))))),
            Value::UserData(userdata) if userdata.is::<node::Drive>() => Ok(Self(Axis::bare(AxisTarget::Drive(node::Drive::from_lua(value.clone(), lua)?.0)))),
            Value::UserData(userdata) if userdata.is::<node::Axis>() => Ok(Self(node::Axis::from_lua(value.clone(), lua)?.0)),
            other => Err(LuaError::runtime(format!(
                "expected a parameter name, a drive or `da.axis`, got {}",
                node::describe(other)
            ))),
        }
    }
}

fn axis(_: &Lua, (target, table): (AxisArgument, Option<Table>)) -> LuaResult<node::Axis> {
    const OWNER: &str = "da.axis";

    let mut options = Options::new(OWNER, table);
    let positive = options.take::<String>("positive")?;
    let negative = options.take::<String>("negative")?;
    options.finish()?;

    let mut axis = target.0;
    if positive.is_some() {
        axis.positive = positive;
    }
    if negative.is_some() {
        axis.negative = negative;
    }
    Ok(node::Axis(axis))
}

fn submenu(lua: &Lua, (name, items): (String, Table)) -> LuaResult<node::MenuItem> {
    Ok(node::MenuItem(MenuItem::SubMenu {
        name,
        items: list::collect::<node::MenuItem>(lua, "da.submenu", &items)?
            .into_iter()
            .map(node::MenuItem::into_inner)
            .collect(),
    }))
}

fn toggle(_: &Lua, (name, drive): (String, node::Drive)) -> LuaResult<node::MenuItem> {
    Ok(node::MenuItem(MenuItem::Toggle {
        name,
        drive: drive.into_inner(),
    }))
}

fn button(_: &Lua, (name, drive): (String, node::Drive)) -> LuaResult<node::MenuItem> {
    Ok(node::MenuItem(MenuItem::Button {
        name,
        drive: drive.into_inner(),
    }))
}

fn radial(_: &Lua, (name, axis): (String, AxisArgument)) -> LuaResult<node::MenuItem> {
    Ok(node::MenuItem(MenuItem::Radial { name, axis: Box::new(axis.0) }))
}

fn two_axis(_: &Lua, (name, table): (String, Table)) -> LuaResult<node::MenuItem> {
    const OWNER: &str = "da.two_axis";

    let mut options = Options::new(OWNER, Some(table));
    let horizontal = needed(OWNER, "horizontal", options.take::<AxisArgument>("horizontal")?)?;
    let vertical = needed(OWNER, "vertical", options.take::<AxisArgument>("vertical")?)?;
    options.finish()?;

    Ok(node::MenuItem(MenuItem::TwoAxis {
        name,
        axes: Box::new(TwoAxes { horizontal, vertical }),
    }))
}

fn four_axis(_: &Lua, (name, table): (String, Table)) -> LuaResult<node::MenuItem> {
    const OWNER: &str = "da.four_axis";

    let mut options = Options::new(OWNER, Some(table));
    let up = needed(OWNER, "up", options.take::<AxisArgument>("up")?)?;
    let down = needed(OWNER, "down", options.take::<AxisArgument>("down")?)?;
    let left = needed(OWNER, "left", options.take::<AxisArgument>("left")?)?;
    let right = needed(OWNER, "right", options.take::<AxisArgument>("right")?)?;
    options.finish()?;

    Ok(node::MenuItem(MenuItem::FourAxis {
        name,
        axes: Box::new(FourAxes { up, down, left, right }),
    }))
}

fn needed(owner: &'static str, label: &str, written: Option<AxisArgument>) -> LuaResult<Axis> {
    written
        .map(|axis| axis.0)
        .ok_or_else(|| LuaError::runtime(format!("{owner}: axis `{label}` is needed")))
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::{
        core::resolution::Unresolved,
        decl::behavior::Drive,
        lua::testing::{eval, eval_error},
    };

    fn item_of(expression: &str) -> MenuItem {
        let (lua, value) = eval(expression);
        node::MenuItem::from_lua(value, &lua).expect("a menu item should be built").0
    }

    fn parameter(name: &str) -> AxisTarget {
        AxisTarget::Parameter(Unresolved::new(name.into()))
    }

    #[rstest]
    fn a_submenu_holds_the_items_written_in_it() {
        let item = item_of(
            "da.submenu('Face', {\
             da.toggle('Hat', da.drive_switch('Hat')),\
             da.button('Wave', da.drive_group('Gesture', 'wave')),\
             })",
        );

        let MenuItem::SubMenu { name, items } = item else {
            panic!("expected a submenu");
        };
        assert_eq!(name, "Face");
        assert_eq!(items.iter().map(MenuItem::name).collect::<Vec<_>>(), ["Hat", "Wave"]);
        assert!(matches!(items[0], MenuItem::Toggle { .. }));
        assert!(matches!(items[1], MenuItem::Button { .. }));
    }

    #[rstest]
    fn a_toggle_and_a_button_carry_their_drive() {
        let MenuItem::Toggle { drive, .. } = item_of("da.toggle('Hat', da.drive_switch('Hat', true))") else {
            panic!("expected a toggle");
        };
        assert_eq!(
            drive,
            Drive::Switch {
                layer: Unresolved::new("Hat".into()),
                value: Some(true),
            },
        );

        let MenuItem::Button { drive, .. } = item_of("da.button('Wave', da.drive_group('Gesture', 'wave'))") else {
            panic!("expected a button");
        };
        assert_eq!(
            drive,
            Drive::Group {
                layer: Unresolved::new("Gesture".into()),
                option: "wave".into(),
            },
        );
    }

    #[rstest]
    fn a_menu_item_needs_a_drive_rather_than_a_name() {
        let message = eval_error("da.toggle('Hat', 'Hat')");
        assert!(message.contains("expected drive, got string"), "{message}");
    }

    #[rstest]
    fn a_radial_axis_is_written_as_a_parameter_name() {
        let MenuItem::Radial { axis, .. } = item_of("da.radial('Blend', 'BlendAmount')") else {
            panic!("expected a radial");
        };

        assert_eq!(axis.target, parameter("BlendAmount"));
        assert_eq!(axis.positive, None);
        assert_eq!(axis.negative, None);
    }

    #[rstest]
    fn a_radial_axis_is_written_as_a_puppet_drive() {
        let MenuItem::Radial { axis, .. } = item_of("da.radial('Wink', da.drive_puppet('Wink'))") else {
            panic!("expected a radial");
        };

        assert_eq!(
            axis.target,
            AxisTarget::Drive(Drive::Puppet {
                layer: Unresolved::new("Wink".into()),
                value: None,
            }),
        );
    }

    #[rstest]
    fn an_axis_takes_the_labels_of_its_ends() {
        let MenuItem::Radial { axis, .. } = item_of("da.radial('Wink', da.axis('WinkAmount', { positive = 'right', negative = 'left' }))") else {
            panic!("expected a radial");
        };

        assert_eq!(axis.target, parameter("WinkAmount"));
        assert_eq!(axis.positive.as_deref(), Some("right"));
        assert_eq!(axis.negative.as_deref(), Some("left"));
    }

    #[rstest]
    fn a_two_axis_item_names_both_of_its_axes() {
        let MenuItem::TwoAxis { name, axes } = item_of("da.two_axis('Look', { horizontal = 'LookX', vertical = da.axis('LookY', { positive = 'up' }) })")
        else {
            panic!("expected a two axis item");
        };

        assert_eq!(name, "Look");
        assert_eq!(axes.horizontal.target, parameter("LookX"));
        assert_eq!(axes.vertical.target, parameter("LookY"));
        assert_eq!(axes.vertical.positive.as_deref(), Some("up"));
    }

    #[rstest]
    fn a_four_axis_item_names_every_direction() {
        let MenuItem::FourAxis { axes, .. } = item_of("da.four_axis('Move', { up = 'U', down = 'D', left = 'L', right = 'R' })") else {
            panic!("expected a four axis item");
        };

        assert_eq!(axes.up.target, parameter("U"));
        assert_eq!(axes.down.target, parameter("D"));
        assert_eq!(axes.left.target, parameter("L"));
        assert_eq!(axes.right.target, parameter("R"));
    }

    #[rstest]
    fn a_puppet_item_needs_every_axis_it_has() {
        let message = eval_error("da.two_axis('Look', { horizontal = 'LookX' })");
        assert!(message.contains("da.two_axis: axis `vertical` is needed"), "{message}");

        let message = eval_error("da.four_axis('Move', { up = 'U', down = 'D', left = 'L' })");
        assert!(message.contains("da.four_axis: axis `right` is needed"), "{message}");
    }

    #[rstest]
    fn a_puppet_item_rejects_an_axis_it_does_not_have() {
        let message = eval_error("da.two_axis('Look', { horizontal = 'X', vertical = 'Y', diagonal = 'D' })");
        assert!(message.contains("da.two_axis: unknown option `diagonal`"), "{message}");
    }

    #[rstest]
    fn an_axis_target_must_be_a_name_a_drive_or_an_axis() {
        let message = eval_error("da.radial('Blend', 42)");
        assert!(message.contains("expected a parameter name, a drive or `da.axis`, got number"), "{message}");
    }

    #[rstest]
    fn the_menu_block_collects_items_in_order() {
        let declared = crate::lua::testing::avatar(
            "local da = require 'declavatar'\nreturn da.avatar({ menu = { da.toggle('Hat', da.drive_switch('Hat')), false, da.radial('Blend', 'B') } })\n",
        );

        assert_eq!(declared.menu.iter().map(MenuItem::name).collect::<Vec<_>>(), ["Hat", "Blend"]);
    }
}
