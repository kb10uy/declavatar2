use crate::{core::resolution::Unresolved, decl::behavior::Drive};

/// Entry of the `menu` block.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    SubMenu { name: String, items: Vec<MenuItem> },
    Toggle { name: String, drive: Drive },
    Button { name: String, drive: Drive },
    Radial { name: String, target: AxisTarget },
    TwoAxis { name: String, axes: Box<TwoAxes> },
    FourAxis { name: String, axes: Box<FourAxes> },
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

/// Axes of a two-axis puppet.
#[derive(Debug, Clone, PartialEq)]
pub struct TwoAxes {
    pub horizontal: Axis,
    pub vertical: Axis,
}

/// Directions of a four-axis puppet.
#[derive(Debug, Clone, PartialEq)]
pub struct FourAxes {
    pub up: Direction,
    pub down: Direction,
    pub left: Direction,
    pub right: Direction,
}

/// One direction of a four-axis puppet, which shows a single label at its end.
#[derive(Debug, Clone, PartialEq)]
pub struct Direction {
    pub target: AxisTarget,
    pub label: Option<String>,
}

/// One axis of a two-axis puppet, with optional labels for its ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    pub target: AxisTarget,
    pub positive: Option<String>,
    pub negative: Option<String>,
}

impl Axis {
    pub fn bare(target: AxisTarget) -> Self {
        Self {
            target,
            positive: None,
            negative: None,
        }
    }
}

/// What an axis moves.
#[derive(Debug, Clone, PartialEq)]
pub enum AxisTarget {
    Parameter(Unresolved<String>),
    Drive(Drive),
}
