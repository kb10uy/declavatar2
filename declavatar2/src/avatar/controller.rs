use crate::{
    avatar::behavior::Behavior,
    core::{
        external::Extern,
        phase::{Compiled, Phase},
    },
    unity::{
        animation::InlineAnimation,
        animator::{AnimatorParameter, BlendTreeType, MergeMode, PathMode},
        external::Asset,
    },
    vrchat::playable_layer::PlayableLayer,
};

/// Reference to an animator parameter whose existence and type have been checked.
pub type ParameterRef = <Compiled as Phase>::ParameterRef;

/// A compiled controller together with how the client applies it to a playable layer.
///
/// Object paths inside the controller are read against the avatar root or, with `PathMode::Relative`,
/// against a root the client supplies. A bound object written in a script is only a path, so the same
/// path used from controllers of different modes names different objects.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayableController {
    pub playable: PlayableLayer,
    pub mode: MergeMode,
    pub priority: i32,
    pub path_mode: PathMode,

    /// Avatar mask the controller is applied with, if the script gave one.
    pub mask: Option<Extern<Asset>>,

    pub controller: AnimatorController,
}

/// Compiled animator controller.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnimatorController {
    /// Every animator parameter the layers refer to, including provided and generated ones.
    pub parameters: Vec<AnimatorParameter>,

    pub layers: Vec<AnimatorLayer>,
}

/// One layer of an animator controller. States and transitions refer to each other by index.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimatorLayer {
    pub name: String,
    pub default_state: Option<usize>,
    pub states: Vec<AnimatorState>,
    pub transitions: Vec<AnimatorTransition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimatorState {
    pub name: String,
    pub motion: Option<Motion>,
    pub playback: Playback,
    pub write_defaults: bool,
    pub behaviors: Vec<Behavior>,
}

/// Playback settings of a state, which Unity keeps on the state rather than on its motion.
#[derive(Debug, Clone, PartialEq)]
pub struct Playback {
    pub speed: f64,
    pub speed_by: Option<ParameterRef>,
    pub time_by: Option<ParameterRef>,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            speed: 1.0,
            speed_by: None,
            time_by: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Motion {
    Clip(Clip),
    BlendTree(BlendTree),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Clip {
    /// Clip the client generates from the given curves.
    Inline(InlineAnimation<Compiled>),

    /// Clip that already exists as a Unity asset.
    External(Extern<Asset>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlendTree {
    Parametric(ParametricBlendTree),
    Direct(DirectBlendTree),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParametricBlendTree {
    pub tree_type: BlendTreeType,
    pub x: ParameterRef,
    pub y: Option<ParameterRef>,
    pub fields: Vec<ParametricField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParametricField {
    /// Threshold for a linear tree, which uses the first component only, or the position for a 2D tree.
    pub position: [f64; 2],
    pub speed: f64,
    pub motion: Motion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirectBlendTree {
    pub fields: Vec<DirectField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirectField {
    pub weight_by: ParameterRef,
    pub speed: f64,
    pub motion: Motion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimatorTransition {
    pub from: TransitionSource,
    pub to: TransitionTarget,
    pub duration: f64,
    pub conditions: Vec<AnimatorCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionSource {
    Entry,
    State(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionTarget {
    State(usize),
    Exit,
}

/// Condition in the form Unity's animator understands. The comparison matches the parameter type.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatorCondition {
    If(ParameterRef),
    IfNot(ParameterRef),
    Equals(ParameterRef, i64),
    NotEqual(ParameterRef, i64),
    Greater(ParameterRef, f64),
    Less(ParameterRef, f64),
}
