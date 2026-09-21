use crate::core::external::{ExternKind, ExternTable};

/// Path to a GameObject, such as `Armature/Hips`.
///
/// It starts at the avatar root, or at the root the client supplies when the controller that
/// refers to it uses relative paths. The table only interns the string; which root applies is
/// decided per controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectPath {}

impl ExternKind for ObjectPath {
    type Value = String;
}

/// Fully qualified Unity component type name, such as `UnityEngine.Light`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentType {}

impl ExternKind for ComponentType {
    type Value = String;
}

/// Unity asset located by `AssetLocator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Asset {}

impl ExternKind for Asset {
    type Value = AssetLocator;
}

/// How the client should find a Unity asset.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssetLocator {
    /// By asset GUID.
    Guid(String),

    /// By asset path from the project root.
    Path(String),

    /// By asset name and its fully qualified type name.
    Named { asset_type: String, name: String },
}

/// Everything an avatar asks the client for: every external reference table, and whether
/// a root for relative paths has to be given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Externals {
    pub object_paths: ExternTable<ObjectPath>,
    pub component_types: ExternTable<ComponentType>,
    pub assets: ExternTable<Asset>,

    /// Whether any controller uses `PathMode::Relative`, so the client must supply the root its paths start at.
    pub needs_relative_root: bool,
}
