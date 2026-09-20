use crate::{
    core::resolution::{SourceLocation, Unresolved},
    decl::behavior::{Animation, Behavior},
    unity::{external::AssetLocator, value::AnimatedValue},
};

pub use crate::unity::animator::BlendTreeType;

/// Layer written as a state machine.
#[derive(Debug, Clone, PartialEq)]
pub struct RawLayer {
    pub name: String,
    pub default_state: Option<Unresolved<String>>,
    pub states: Vec<RawState>,

    /// Every transition of this layer, including the ones written inside a state.
    pub transitions: Vec<RawTransition>,

    pub at: Option<SourceLocation>,
}

/// One state of a `RawLayer`.
#[derive(Debug, Clone, PartialEq)]
pub struct RawState {
    pub name: String,
    pub motion: Option<Motion>,
    pub behaviors: Vec<Behavior>,
    pub at: Option<SourceLocation>,
}

/// Transition between two states of a `RawLayer`.
#[derive(Debug, Clone, PartialEq)]
pub struct RawTransition {
    pub from: Unresolved<String>,
    pub to: Unresolved<String>,
    pub duration: Option<f64>,
    pub conditions: Vec<Condition>,
}

/// What a state plays.
#[derive(Debug, Clone, PartialEq)]
pub enum Motion {
    /// Clip generated from the written targets.
    Clip {
        options: ClipOptions,
        animation: Animation,
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
