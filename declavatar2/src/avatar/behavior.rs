use crate::{
    core::phase::Compiled,
    unity::state::GenericStateBehavior,
    vrchat::state_behaviour::{LayerControl, LocomotionControl, ParameterDrive, PlayAudio, PlayableLayerControl, TemporaryPoseSpace, TrackingControl},
};

/// State behavior attached to a compiled state.
#[derive(Debug, Clone, PartialEq)]
pub enum Behavior {
    ParameterDrive(ParameterDrive<Compiled>),
    TrackingControl(TrackingControl),
    Generic(GenericStateBehavior<Compiled>),
    LayerControl(LayerControl<Compiled>),
    LocomotionControl(LocomotionControl),
    TemporaryPoseSpace(TemporaryPoseSpace),
    PlayableLayerControl(PlayableLayerControl),
    PlayAudio(PlayAudio<Compiled>),
}
