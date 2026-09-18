use crate::{core::resolution::Unresolved, decl::behavior::Drive};

/// Entry of the `menu` block.
#[derive(Debug, Clone)]
pub enum MenuItem {
    SubMenu { name: String, items: Vec<MenuItem> },
    Toggle { name: String, drive: Drive },
    Button { name: String, drive: Drive },
    Radial { name: String, axis: Box<Axis> },
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
#[derive(Debug, Clone)]
pub struct TwoAxes {
    pub horizontal: Axis,
    pub vertical: Axis,
}

/// Axes of a four-axis puppet.
#[derive(Debug, Clone)]
pub struct FourAxes {
    pub up: Axis,
    pub down: Axis,
    pub left: Axis,
    pub right: Axis,
}

/// One axis of a puppet menu item, with optional labels for its ends.
/// A four-axis item uses `positive` as the label of its direction.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub enum AxisTarget {
    Parameter(Unresolved<String>),
    Drive(Drive),
}
