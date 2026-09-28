use crate::{
    core::resolution::{SourceLocation, Unresolved},
    decl::{
        behavior::{Animation, Content},
        raw::RawMachine,
    },
    unity::{animator::LayerBlending, external::AssetLocator},
};

/// Layer inside a controller.
#[derive(Debug, Clone, PartialEq)]
pub enum Layer {
    Group(GroupLayer),
    Switch(SwitchLayer),
    Puppet(PuppetLayer),
    Blend(BlendLayer),
    Raw(RawLayer),
}

impl Layer {
    pub fn name(&self) -> &str {
        match self {
            Layer::Group(layer) => &layer.name,
            Layer::Switch(layer) => &layer.name,
            Layer::Puppet(layer) => &layer.name,
            Layer::Blend(layer) => &layer.name,
            Layer::Raw(layer) => &layer.machine.name,
        }
    }

    pub fn at(&self) -> Option<&SourceLocation> {
        match self {
            Layer::Group(layer) => layer.at.as_ref(),
            Layer::Switch(layer) => layer.at.as_ref(),
            Layer::Puppet(layer) => layer.at.as_ref(),
            Layer::Blend(layer) => layer.at.as_ref(),
            Layer::Raw(layer) => layer.machine.at.as_ref(),
        }
    }

    pub fn settings(&self) -> &LayerSettings {
        match self {
            Layer::Group(layer) => &layer.settings,
            Layer::Switch(layer) => &layer.settings,
            Layer::Puppet(layer) => &layer.settings,
            Layer::Blend(layer) => &layer.settings,
            Layer::Raw(layer) => &layer.settings,
        }
    }
}

/// How a layer is applied on top of the layers before it, as written in its options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerSettings {
    pub weight: Option<f64>,
    pub blending: Option<LayerBlending>,
    pub mask: Option<Unresolved<AssetLocator>>,
}

/// Layer written as a state machine, which is its root machine.
#[derive(Debug, Clone, PartialEq)]
pub struct RawLayer {
    pub settings: LayerSettings,
    pub machine: RawMachine,
}

/// Layer that switches between mutually exclusive options.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupLayer {
    pub name: String,
    pub settings: LayerSettings,
    pub driven_by: Option<Unresolved<String>>,
    pub symmetric: Option<bool>,
    pub default: Option<Content>,
    pub options: Vec<GroupOption>,
    pub at: Option<SourceLocation>,
}

/// One option of a `GroupLayer`.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupOption {
    pub name: String,
    pub content: Content,
    pub at: Option<SourceLocation>,
}

/// Layer that has exactly two states.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchLayer {
    pub name: String,
    pub settings: LayerSettings,
    pub driven_by: Option<Unresolved<String>>,
    pub content: SwitchContent,
    pub at: Option<SourceLocation>,
}

/// How the two states of a `SwitchLayer` are written.
#[derive(Debug, Clone, PartialEq)]
pub enum SwitchContent {
    /// Toggle list: the written values are the on state, and the transform zeroes them for the off state.
    Toggle(Content),

    /// Both states are written explicitly.
    Sides { off: Content, on: Content },
}

/// Layer that interpolates its targets along a parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct PuppetLayer {
    pub name: String,
    pub settings: LayerSettings,
    pub driven_by: Option<Unresolved<String>>,
    pub keyframes: Vec<PuppetKeyframe>,
    pub at: Option<SourceLocation>,
}

/// One keyframe of a `PuppetLayer`.
#[derive(Debug, Clone, PartialEq)]
pub struct PuppetKeyframe {
    pub time: f64,
    pub animation: Animation,
}

/// Layer that merges its children into one direct blend tree.
#[derive(Debug, Clone, PartialEq)]
pub struct BlendLayer {
    pub name: String,
    pub settings: LayerSettings,
    pub puppets: Vec<PuppetLayer>,
    pub at: Option<SourceLocation>,
}
