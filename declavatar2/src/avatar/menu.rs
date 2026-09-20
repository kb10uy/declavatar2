use crate::{avatar::controller::ParameterRef, unity::value::AnimatedValue};

/// Compiled expression menu control.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    SubMenu {
        name: String,
        items: Vec<MenuItem>,
    },
    Toggle {
        name: String,
        parameter: ParameterRef,
        value: AnimatedValue<()>,
    },
    Button {
        name: String,
        parameter: ParameterRef,
        value: AnimatedValue<()>,
    },
    Radial {
        name: String,
        axis: MenuAxis,
    },
    TwoAxis {
        name: String,
        horizontal: MenuAxis,
        vertical: MenuAxis,
    },
    FourAxis {
        name: String,
        up: MenuAxis,
        down: MenuAxis,
        left: MenuAxis,
        right: MenuAxis,
    },
}

impl MenuItem {
    /// How many controls one menu can hold.
    pub const CAPACITY: usize = 8;

    pub fn name(&self) -> &str {
        match self {
            MenuItem::SubMenu { name, .. }
            | MenuItem::Toggle { name, .. }
            | MenuItem::Button { name, .. }
            | MenuItem::Radial { name, .. }
            | MenuItem::TwoAxis { name, .. }
            | MenuItem::FourAxis { name, .. } => name,
        }
    }
}

/// Float parameter a puppet control moves, with optional labels for its ends.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuAxis {
    pub parameter: ParameterRef,
    pub positive: Option<String>,
    pub negative: Option<String>,
}
