use std::fmt::Debug;

/// Traits for AnimatorController StateBehaviour definitions.
///
/// Note: the spelling of `StateBehavior` is intentionally different from `StateBehaviour`.
pub trait StateBehavior: Debug {
    /// Returns the identification name of the state behavior.
    /// This takes self receiver to keep itself dyn-compatible.
    fn name(&self) -> &'static str;

    /// Clones this definition.
    /// This is apart of the `Clone` trait due to its `Sized` requirement, which leads to dyn-incompatible.
    fn clone(&self) -> Box<dyn StateBehavior>;

    /// Serializes this definition data into a byte array.
    /// Its representation relies on each state behavior's specific data layout.
    fn serialize(&self) -> Vec<u8>;
}

impl Clone for Box<dyn StateBehavior> {
    fn clone(&self) -> Self {
        self.as_ref().clone()
    }
}
