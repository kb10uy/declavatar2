/// Playable layer of a VRChat avatar descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayableLayer {
    Base,
    Additive,
    Gesture,
    Action,
    Fx,
    Sitting,
    TPose,
    IkPose,
}
