use serde::{Deserialize, Serialize};

use crate::core::external::{ExternKind, ExternTable};

/// Path to a GameObject relative to the avatar root, such as `Armature/Hips`.
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AssetLocator {
    /// By asset GUID.
    Guid(String),

    /// By asset path from the project root.
    Path(String),

    /// By asset name and its fully qualified type name.
    Named { asset_type: String, name: String },
}

/// Every external reference table of an avatar.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Externals {
    pub object_paths: ExternTable<ObjectPath>,
    pub component_types: ExternTable<ComponentType>,
    pub assets: ExternTable<Asset>,
}
