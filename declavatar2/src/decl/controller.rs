use crate::{
    core::resolution::{SourceLocation, Unresolved},
    decl::layer::Layer,
    unity::{
        animator::{MergeMode, PathMode},
        external::AssetLocator,
    },
    vrchat::playable_layer::PlayableLayer,
};

/// Entry of the `controllers` block: the layers bound for one playable layer and how they are applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Controller {
    pub playable: PlayableLayer,
    pub mode: Option<MergeMode>,
    pub priority: Option<i32>,
    pub path_mode: Option<PathMode>,
    pub mask: Option<Unresolved<AssetLocator>>,
    pub layers: Vec<Layer>,
    pub at: Option<SourceLocation>,
}

impl Controller {
    /// A controller with every option left unwritten.
    pub fn new(playable: PlayableLayer, layers: Vec<Layer>) -> Self {
        Self {
            playable,
            mode: None,
            priority: None,
            path_mode: None,
            mask: None,
            layers,
            at: None,
        }
    }
}
