pub mod animation;
pub mod animator;
pub mod external;
pub mod state;
pub mod value;

pub use animation::{ClipAttributes, Curve, CurveError, FixedAnimationEntry, InlineAnimation, Interpolation, KeyedAnimation, KeyedAnimationEntry, Keyframe};
pub use animator::{
    AnimatedAnimatorProperty, AnimatedAnimatorTarget, AnimatedComponentProperty, AnimatedComponentTarget, AnimatedGameObjectProperty, AnimatedGameObjectTarget,
    AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget, AnimatorParameter, AnimatorParameterType, AnimatorParameterTypeDefault, BlendTreeType,
    MergeMode, PathMode,
};
pub use external::{Asset, AssetLocator, ComponentType, Externals, ObjectPath};
pub use state::{GenericStateBehavior, GenericValue, StateBehavior};
pub use value::{AnimatedValue, AnimatedValueCast, AnimatedValueType};
