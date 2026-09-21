pub mod error;

mod animation;
mod behavior;
mod context;
mod layer;
mod menu;
mod raw;

use crate::{
    avatar::{
        Avatar,
        controller::{AnimatorController, PlayableController},
    },
    decl,
    transform::context::Context,
    unity::animator::PathMode,
};

pub use error::{TransformError, TransformErrorKind, TransformErrors};

/// Compiles a declaration into an avatar.
///
/// The 1st pass collects every declared name, and the 2nd pass resolves references and checks types
/// while it builds the output. Every error found on the way is returned together.
pub fn transform(declaration: &decl::Avatar) -> Result<Avatar, TransformErrors> {
    let (mut context, mut errors) = Context::collect(declaration);

    let mut compiled = Vec::new();
    for controller in &declaration.controllers {
        let mut layers = Vec::new();
        for layer in &controller.layers {
            match layer::compile(&mut context, layer) {
                Ok(layer) => layers.push(layer),
                Err(error) => errors.push(error),
            }
        }
        let mask = controller.mask.as_ref().map(|mask| context.externals.assets.intern(mask.clone()));
        compiled.push((controller, mask, layers));
    }
    let menu = menu::compile(&context, &declaration.menu, &mut errors);

    if !errors.is_empty() {
        return Err(TransformErrors(errors));
    }

    let parameters = context.parameters.animator_parameters();
    let controllers: Vec<_> = compiled
        .into_iter()
        .map(|(controller, mask, layers)| PlayableController {
            playable: controller.playable,
            mode: controller.mode.unwrap_or_default(),
            priority: controller.priority.unwrap_or(0),
            path_mode: controller.path_mode.unwrap_or_default(),
            mask,
            controller: AnimatorController {
                parameters: parameters.clone(),
                layers,
            },
        })
        .collect();
    context.externals.needs_relative_root = controllers.iter().any(|controller| controller.path_mode == PathMode::Relative);

    Ok(Avatar {
        expression_parameters: context.parameters.expression_parameters(),
        controllers,
        menu,
        externals: context.externals,
    })
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::{
        core::resolution::Unresolved,
        decl::{
            controller::Controller,
            layer::{GroupLayer, Layer},
        },
        unity::{animator::MergeMode, external::AssetLocator},
        vrchat::playable_layer::PlayableLayer,
    };

    fn group(name: &str) -> Layer {
        Layer::Group(GroupLayer {
            name: name.into(),
            driven_by: None,
            symmetric: None,
            default: None,
            options: vec![],
            at: None,
        })
    }

    fn int_parameter(name: &str) -> decl::Parameter {
        decl::Parameter::Primitive(decl::PrimitiveParameter {
            name: name.into(),
            value: decl::PrimitiveParameterValue::Int { default: None, width: None },
            scope: None,
            save: None,
            at: None,
        })
    }

    #[rstest]
    fn an_unwritten_option_takes_its_default() {
        let avatar = transform(&decl::Avatar {
            controllers: vec![Controller::new(PlayableLayer::Fx, vec![])],
            ..decl::Avatar::default()
        })
        .expect("declaration should compile");

        assert_eq!(avatar.controllers.len(), 1);
        let controller = &avatar.controllers[0];
        assert_eq!(controller.playable, PlayableLayer::Fx);
        assert_eq!(controller.mode, MergeMode::Append);
        assert_eq!(controller.priority, 0);
        assert_eq!(controller.path_mode, PathMode::Absolute);
        assert_eq!(controller.mask, None);
        assert!(!avatar.externals.needs_relative_root);
    }

    #[rstest]
    fn every_controller_is_compiled_in_order_with_the_whole_parameter_list() {
        let avatar = transform(&decl::Avatar {
            parameters: vec![int_parameter("Left"), int_parameter("Right"), int_parameter("A")],
            controllers: vec![
                Controller {
                    mode: Some(MergeMode::Replace),
                    priority: Some(-5),
                    mask: Some(Unresolved::new(AssetLocator::Path("Assets/Hands.mask".into()))),
                    ..Controller::new(PlayableLayer::Gesture, vec![group("Left"), group("Right")])
                },
                Controller {
                    path_mode: Some(PathMode::Relative),
                    ..Controller::new(PlayableLayer::Fx, vec![group("A")])
                },
            ],
            ..decl::Avatar::default()
        })
        .expect("declaration should compile");

        let names: Vec<Vec<_>> = avatar
            .controllers
            .iter()
            .map(|controller| controller.controller.layers.iter().map(|layer| layer.name.as_str()).collect())
            .collect();
        assert_eq!(names, [vec!["Left", "Right"], vec!["A"]]);

        let gesture = &avatar.controllers[0];
        assert_eq!(gesture.playable, PlayableLayer::Gesture);
        assert_eq!(gesture.mode, MergeMode::Replace);
        assert_eq!(gesture.priority, -5);
        let mask = gesture.mask.expect("mask should be interned");
        assert_eq!(avatar.externals.assets.get(mask).value, AssetLocator::Path("Assets/Hands.mask".into()));

        let fx = &avatar.controllers[1];
        assert_eq!(fx.path_mode, PathMode::Relative);
        assert!(avatar.externals.needs_relative_root);

        for controller in &avatar.controllers {
            let names: Vec<_> = controller.controller.parameters.iter().map(|parameter| parameter.name.as_str()).collect();
            assert_eq!(names, ["Left", "Right", "A"]);
        }
    }

    #[rstest]
    fn layer_names_are_unique_across_controllers() {
        let errors = transform(&decl::Avatar {
            parameters: vec![int_parameter("Same")],
            controllers: vec![
                Controller::new(PlayableLayer::Fx, vec![group("Same")]),
                Controller::new(PlayableLayer::Gesture, vec![group("Same")]),
            ],
            ..decl::Avatar::default()
        })
        .expect_err("the duplicate should be reported");

        assert_eq!(errors.0, vec![TransformErrorKind::DuplicateLayer { name: "Same".into() }.at(None)]);
    }
}
