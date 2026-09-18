use crate::{
    core::resolution::{SourceLocation, Unresolved},
    decl::{
        behavior::{Animation, Content},
        raw::RawLayer,
    },
};

/// Entry of the `fx_controller` block.
#[derive(Debug, Clone)]
pub enum Layer {
    Group(GroupLayer),
    Switch(SwitchLayer),
    Puppet(PuppetLayer),
    Raw(RawLayer),
}

impl Layer {
    pub fn name(&self) -> &str {
        match self {
            Layer::Group(layer) => &layer.name,
            Layer::Switch(layer) => &layer.name,
            Layer::Puppet(layer) => &layer.name,
            Layer::Raw(layer) => &layer.name,
        }
    }

    pub fn at(&self) -> Option<&SourceLocation> {
        match self {
            Layer::Group(layer) => layer.at.as_ref(),
            Layer::Switch(layer) => layer.at.as_ref(),
            Layer::Puppet(layer) => layer.at.as_ref(),
            Layer::Raw(layer) => layer.at.as_ref(),
        }
    }
}

/// Layer that switches between mutually exclusive options.
#[derive(Debug, Clone)]
pub struct GroupLayer {
    pub name: String,
    pub driven_by: Option<Unresolved<String>>,
    pub default: Option<Content>,
    pub options: Vec<GroupOption>,
    pub at: Option<SourceLocation>,
}

/// One option of a `GroupLayer`.
#[derive(Debug, Clone)]
pub struct GroupOption {
    pub name: String,
    pub index: Option<u32>,
    pub content: Content,
    pub at: Option<SourceLocation>,
}

/// Layer that has exactly two states.
#[derive(Debug, Clone)]
pub struct SwitchLayer {
    pub name: String,
    pub source: Option<SwitchSource>,
    pub content: SwitchContent,
    pub at: Option<SourceLocation>,
}

/// What decides the state of a `SwitchLayer`.
#[derive(Debug, Clone, PartialEq)]
pub enum SwitchSource {
    Parameter(Unresolved<String>),
    Gate(Unresolved<String>),
}

/// How the two states of a `SwitchLayer` are written.
#[derive(Debug, Clone)]
pub enum SwitchContent {
    /// Toggle list: the written values are the on state, and the transform zeroes them for the off state.
    Toggle(Content),

    /// Both states are written explicitly.
    Sides { off: Content, on: Content },
}

/// Layer that interpolates its targets along a parameter.
#[derive(Debug, Clone)]
pub struct PuppetLayer {
    pub name: String,
    pub driven_by: Option<Unresolved<String>>,
    pub keyframes: Vec<PuppetKeyframe>,
    pub at: Option<SourceLocation>,
}

/// One keyframe of a `PuppetLayer`.
#[derive(Debug, Clone)]
pub struct PuppetKeyframe {
    pub time: f64,
    pub animation: Animation,
}
