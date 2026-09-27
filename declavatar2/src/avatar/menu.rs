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
        parameter: ParameterRef,
    },
    TwoAxis {
        name: String,
        horizontal: MenuAxis,
        vertical: MenuAxis,
    },
    FourAxis {
        name: String,
        up: MenuDirection,
        down: MenuDirection,
        left: MenuDirection,
        right: MenuDirection,
    },
}

impl MenuItem {
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

/// Float parameter one axis of a two-axis puppet moves, with optional labels for its ends.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuAxis {
    pub parameter: ParameterRef,
    pub positive: Option<String>,
    pub negative: Option<String>,
}

/// One direction of a four-axis puppet: the float parameter it moves, and the one label shown at its end.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuDirection {
    pub parameter: ParameterRef,
    pub label: Option<String>,
}
