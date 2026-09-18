use crate::{
    core::{phase::Declared, resolution::Unresolved, value_set::ValueSet},
    unity::{animation::FixedAnimationEntry, state::GenericStateBehavior, value::AnimatedValue},
    vrchat::state_behaviour::TrackingControl,
};

/// Set of animated targets and their values written in one place of the script.
pub type Animation = ValueSet<FixedAnimationEntry<Declared>>;

/// What a group option, a switch side, or a raw state holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Content {
    pub animation: Animation,
    pub behaviors: Vec<Behavior>,
}

impl Content {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Non-animation effect attached to a state.
#[derive(Debug, Clone, PartialEq)]
pub enum Behavior {
    Drive(Drive),
    TrackingControl(TrackingControl),

    /// State behavior of a type declavatar2 does not know about.
    Generic(GenericStateBehavior),
}

/// Parameter drive written as `da.drive_*`.
/// Layer-targeting drives are turned into concrete parameter values by the transform.
#[derive(Debug, Clone, PartialEq)]
pub enum Drive {
    Group { layer: Unresolved<String>, option: String },
    Switch { layer: Unresolved<String>, value: Option<bool> },
    Puppet { layer: Unresolved<String>, value: Option<f64> },
    Parameter { parameter: Unresolved<String>, value: AnimatedValue<()> },
}
