use std::collections::HashMap;

use crate::{
    core::phase::Phase,
    unity::{state::StateBehavior, value::AnimatedValue},
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

    fn serialize(&self) -> Vec<u8> {
        todo!();
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

    fn serialize(&self) -> Vec<u8> {
        todo!();
    }
}
