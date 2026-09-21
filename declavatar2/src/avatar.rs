pub mod behavior;
pub mod controller;
pub mod menu;

use crate::{unity::external::Externals, vrchat::expr_parameter::ExpressionParameter};

pub use behavior::Behavior;
pub use controller::{
    AnimatorCondition, AnimatorController, AnimatorLayer, AnimatorState, AnimatorTransition, BlendTree, Clip, DirectBlendTree, DirectField, Motion,
    ParameterRef, ParametricBlendTree, ParametricField, PlayableController, Playback, TransitionSource, TransitionTarget,
};
pub use menu::{MenuAxis, MenuItem};

/// Compiled avatar data: what the transform produces and the client consumes.
/// Everything here is globally consistent, and every external reference is interned into `externals`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Avatar {
    /// Parameters exposed as VRChat expression parameters, in declaration order.
    pub expression_parameters: Vec<ExpressionParameter>,

    /// Generated controllers in declaration order. Every one carries the full animator parameter list.
    pub controllers: Vec<PlayableController>,

    pub menu: Vec<MenuItem>,
    pub externals: Externals,
}
