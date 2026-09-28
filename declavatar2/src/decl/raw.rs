use crate::{
    core::{
        phase::Declared,
        resolution::{SourceLocation, Unresolved},
    },
    decl::behavior::{Animation, Behavior},
    unity::{animation::KeyedAnimation, external::AssetLocator, value::AnimatedValue},
};

pub use crate::unity::animator::BlendTreeType;

/// State machine written with `da.raw.layer` or `da.raw.machine`. A raw layer is its root machine, named after the layer.
///
/// States and nested machines share one namespace per machine, and a name written in a machine refers to
/// something that machine holds directly.
#[derive(Debug, Clone, PartialEq)]
pub struct RawMachine {
    pub name: String,
    pub default_state: Option<Unresolved<String>>,
    pub states: Vec<RawState>,
    pub machines: Vec<RawMachine>,

    /// Every transition of this machine, including the ones written inside its states.
    pub transitions: Vec<RawTransition>,

    pub at: Option<SourceLocation>,
}

/// One state of a `RawMachine`.
#[derive(Debug, Clone, PartialEq)]
pub struct RawState {
    pub name: String,
    pub motion: Option<Motion>,
    pub behaviors: Vec<Behavior>,
    pub at: Option<SourceLocation>,
}

/// Transition between two nodes of one `RawMachine`.
#[derive(Debug, Clone, PartialEq)]
pub struct RawTransition {
    pub from: TransitionSource,
    pub to: TransitionTarget,
    pub duration: Option<f64>,
    pub conditions: Vec<Condition>,
}

/// Where a transition leaves from.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionSource {
    /// The entry of the machine holding the transition.
    Entry,

    /// A state, or the exit of a nested machine, named in the machine holding the transition.
    Node(Unresolved<String>),
}

/// Where a transition leads.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionTarget {
    /// The exit of the machine holding the transition.
    Exit,

    /// A state, or a nested machine entered through its entry, named in the machine holding the transition.
    Node(Unresolved<String>),
}

/// What a state plays.
#[derive(Debug, Clone, PartialEq)]
pub enum Motion {
    /// Clip generated from the written targets.
    Clip {
        options: ClipOptions,
        animation: Animation,
    },

    /// Clip generated from curves over normalized time.
    Keyed {
        options: ClipOptions,
        animation: KeyedAnimation<Declared>,
    },

    /// Clip that already exists as a Unity asset.
    External {
        asset: Unresolved<AssetLocator>,
        options: ClipOptions,
    },

    BlendTree(BlendTree),
}

/// Playback settings of a motion.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipOptions {
    pub speed: Option<f64>,
    pub speed_by: Option<Unresolved<String>>,
    pub time_by: Option<Unresolved<String>>,
}

/// Motion that blends its fields.
#[derive(Debug, Clone, PartialEq)]
pub enum BlendTree {
    /// Fields placed on one or two parameter axes.
    Parametric(ParametricBlendTree),

    /// Fields summed with a weight parameter each.
    Direct(DirectBlendTree),
}

/// Blend tree whose fields are placed on one or two parameter axes.
#[derive(Debug, Clone, PartialEq)]
pub struct ParametricBlendTree {
    pub tree_type: BlendTreeType,
    pub x: Unresolved<String>,
    pub y: Option<Unresolved<String>>,
    pub fields: Vec<BlendTreeField>,
}

/// Blend tree whose fields are summed with a weight parameter each.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectBlendTree {
    pub fields: Vec<DirectBlendTreeField>,
}

/// One field of a `ParametricBlendTree`.
#[derive(Debug, Clone, PartialEq)]
pub struct BlendTreeField {
    pub position: [f64; 2],
    pub motion: Motion,
}

/// One field of a `DirectBlendTree`.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectBlendTreeField {
    pub weight_by: Unresolved<String>,
    pub motion: Motion,
}

/// Condition of a `RawTransition`.
/// The comparison type is determined from the parameter type in the 2nd pass.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Zero(Unresolved<String>),
    NonZero(Unresolved<String>),
    Eq(Unresolved<String>, AnimatedValue<()>),
    Ne(Unresolved<String>, AnimatedValue<()>),
    Gt(Unresolved<String>, AnimatedValue<()>),
    Lt(Unresolved<String>, AnimatedValue<()>),
}

impl Condition {
    pub fn parameter(&self) -> &Unresolved<String> {
        match self {
            Condition::Zero(parameter)
            | Condition::NonZero(parameter)
            | Condition::Eq(parameter, _)
            | Condition::Ne(parameter, _)
            | Condition::Gt(parameter, _)
            | Condition::Lt(parameter, _) => parameter,
        }
    }
}
