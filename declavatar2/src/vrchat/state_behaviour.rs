use std::collections::HashMap;

use crate::unity::{state::StateBehavior, value::AnimatedValue};

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDrive {
    pub target: ParameterDriveTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterDriveTarget {
    Set {
        parameter: String,
        value: AnimatedValue<()>,
    },
    Add {
        parameter: String,
        value: AnimatedValue<()>,
    },
    RandomInt {
        parameter: String,
        range: [i64; 2],
    },
    RandomBool {
        parameter: String,
        chance: f64,
    },
    RandomFloat {
        parameter: String,
        range: [f64; 2],
    },
    Copy {
        from: String,
        to: String,
    },
    RangedCopy {
        from: String,
        from_range: [f64; 2],
        to: String,
        to_range: [f64; 2],
    },
}

impl StateBehavior for ParameterDrive {
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
