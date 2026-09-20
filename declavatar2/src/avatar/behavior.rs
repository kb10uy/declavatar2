use crate::{
    core::phase::Compiled,
    unity::state::GenericStateBehavior,
    vrchat::state_behaviour::{ParameterDrive, TrackingControl},
};

/// State behavior attached to a compiled state.
#[derive(Debug, Clone, PartialEq)]
pub enum Behavior {
    ParameterDrive(ParameterDrive<Compiled>),
    TrackingControl(TrackingControl),
    Generic(GenericStateBehavior),
}
