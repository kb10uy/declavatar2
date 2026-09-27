use std::collections::HashMap;

use crate::{
    core::phase::Phase,
    unity::{state::StateBehavior, value::AnimatedValue},
    vrchat::playable_layer::PlayableLayer,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDrive<Ph: Phase> {
    pub target: ParameterDriveTarget<Ph>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterDriveTarget<Ph: Phase> {
    Set {
        parameter: Ph::ParameterRef,
        value: AnimatedValue<()>,
    },
    Add {
        parameter: Ph::ParameterRef,
        value: AnimatedValue<()>,
    },
    RandomInt {
        parameter: Ph::ParameterRef,
        range: [i64; 2],
    },
    RandomBool {
        parameter: Ph::ParameterRef,
        chance: f64,
    },
    RandomFloat {
        parameter: Ph::ParameterRef,
        range: [f64; 2],
    },
    Copy {
        from: Ph::ParameterRef,
        to: Ph::ParameterRef,
    },
    RangedCopy {
        from: Ph::ParameterRef,
        from_range: [f64; 2],
        to: Ph::ParameterRef,
        to_range: [f64; 2],
    },
}

impl<Ph: Phase> StateBehavior for ParameterDrive<Ph> {
    fn name(&self) -> &'static str {
        "VRCAvatarParameterDriver"
    }

    fn clone(&self) -> Box<dyn StateBehavior> {
        Box::new(Clone::clone(self))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingControl {
    pub values: HashMap<TrackingControlTarget, TrackingControlMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackingControlTarget {
    Head,
    LeftHand,
    RightHand,
    Hip,
    LeftFoot,
    RightFoot,
    LeftFingers,
    RightFingers,
    Eyes,
    Mouth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingControlMode {
    NoChange,
    Tracking,
    Animation,
}

impl StateBehavior for TrackingControl {
    fn name(&self) -> &'static str {
        "VRCAnimatorTrackingControl"
    }

    fn clone(&self) -> Box<dyn StateBehavior> {
        Box::new(Clone::clone(self))
    }
}

/// Playable layer whose weight a state behavior can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendablePlayable {
    Action,
    Fx,
    Gesture,
    Additive,
}

impl BlendablePlayable {
    pub fn of(playable: PlayableLayer) -> Option<Self> {
        match playable {
            PlayableLayer::Action => Some(Self::Action),
            PlayableLayer::Fx => Some(Self::Fx),
            PlayableLayer::Gesture => Some(Self::Gesture),
            PlayableLayer::Additive => Some(Self::Additive),
            PlayableLayer::Base | PlayableLayer::Sitting | PlayableLayer::TPose | PlayableLayer::IkPose => None,
        }
    }
}

/// Blends the weight of one layer toward a goal.
/// The layer lives in the playable layer of the controller that holds the state.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerControl<Ph: Phase> {
    pub layer: Ph::LayerRef,
    pub goal_weight: f64,

    /// Seconds the weight takes to reach the goal.
    pub blend_duration: f64,
}

impl<Ph: Phase> StateBehavior for LayerControl<Ph> {
    fn name(&self) -> &'static str {
        "VRCAnimatorLayerControl"
    }

    fn clone(&self) -> Box<dyn StateBehavior> {
        Box::new(Clone::clone(self))
    }
}

/// Blends the weight of a whole playable layer toward a goal.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayableLayerControl {
    pub playable: BlendablePlayable,
    pub goal_weight: f64,

    /// Seconds the weight takes to reach the goal.
    pub blend_duration: f64,
}

impl StateBehavior for PlayableLayerControl {
    fn name(&self) -> &'static str {
        "VRCPlayableLayerControl"
    }

    fn clone(&self) -> Box<dyn StateBehavior> {
        Box::new(Clone::clone(self))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocomotionControl {
    pub disable_locomotion: bool,
}

impl StateBehavior for LocomotionControl {
    fn name(&self) -> &'static str {
        "VRCAnimatorLocomotionControl"
    }

    fn clone(&self) -> Box<dyn StateBehavior> {
        Box::new(*self)
    }
}

/// Moves the viewpoint to the head (`enter`) or back to where it was (`exit`) after a delay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporaryPoseSpace {
    pub enter: bool,

    /// Whether `delay` is in seconds, rather than a fraction of the state.
    pub fixed_delay: bool,

    pub delay: f64,
}

impl StateBehavior for TemporaryPoseSpace {
    fn name(&self) -> &'static str {
        "VRCAnimatorTemporaryPoseSpace"
    }

    fn clone(&self) -> Box<dyn StateBehavior> {
        Box::new(*self)
    }
}

/// Plays clips on an AudioSource when the state is entered or left.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayAudio<Ph: Phase> {
    /// Object holding the AudioSource, or `None` for the root the controller's paths start at.
    pub source: Option<Ph::ObjectPath>,

    pub order: PlaybackOrder<Ph>,
    pub clips: AudioSetting<Vec<Ph::ObjectRef>>,
    pub volume: AudioSetting<[f64; 2]>,
    pub pitch: AudioSetting<[f64; 2]>,
    pub looping: AudioSetting<bool>,

    /// Seconds between entering the state and playing.
    pub delay: f64,

    pub play_on_enter: bool,
    pub stop_on_enter: bool,
    pub play_on_exit: bool,
    pub stop_on_exit: bool,
}

impl<Ph: Phase> StateBehavior for PlayAudio<Ph> {
    fn name(&self) -> &'static str {
        "VRCAnimatorPlayAudio"
    }

    fn clone(&self) -> Box<dyn StateBehavior> {
        Box::new(Clone::clone(self))
    }
}

/// Which clip plays next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackOrder<Ph: Phase> {
    Random,
    UniqueRandom,
    Roundabout,

    /// The clip at the index an int parameter holds.
    Parameter(Ph::ParameterRef),
}

/// A setting of the AudioSource, and when it is written to the source.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSetting<T> {
    pub value: T,
    pub apply: ApplySettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplySettings {
    Always,
    IfStopped,
    Never,
}
