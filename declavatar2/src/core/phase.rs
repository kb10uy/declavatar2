use std::{fmt::Debug, hash::Hash};

use crate::{
    core::{
        external::Extern,
        resolution::{Resolved, Unresolved},
    },
    unity::{
        AnimatedValueType,
        external::{Asset, AssetLocator, ComponentType, ObjectPath},
    },
};

/// Selects how references inside data structures are represented.
/// Structures generic over `Phase` share one definition between `Declaration` and `Avatar`.
pub trait Phase: 'static + Debug + Copy + Ord + Hash {
    /// Reference to an animator parameter.
    type ParameterRef: Debug + Clone + Eq + Ord;

    /// Path to a GameObject relative to the avatar root.
    type ObjectPath: Debug + Clone + Eq + Ord;

    /// Fully qualified Unity component type name.
    type ComponentType: Debug + Clone + Eq + Ord;

    /// Reference to a Unity asset such as Material or Mesh.
    type ObjectRef: Debug + Clone + Eq + Ord;
}

/// Phase right after the script has been evaluated. Every reference is a bare name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Declared {}

impl Phase for Declared {
    type ParameterRef = Unresolved<String>;
    type ObjectPath = Unresolved<String>;
    type ComponentType = Unresolved<String>;
    type ObjectRef = Unresolved<AssetLocator>;
}

/// Phase after the transformer has run. Script-internal references are resolved,
/// and Unity-side references are interned into `ExternTable`s for the client to verify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Compiled {}

impl Phase for Compiled {
    type ParameterRef = Resolved<String, AnimatedValueType>;
    type ObjectPath = Extern<ObjectPath>;
    type ComponentType = Extern<ComponentType>;
    type ObjectRef = Extern<Asset>;
}
