pub mod error;

mod animation;
mod behavior;
mod context;
mod layer;
mod menu;
mod raw;

use crate::{
    avatar::{Avatar, controller::AnimatorController},
    decl,
    transform::context::Context,
};

pub use error::{TransformError, TransformErrorKind, TransformErrors};

/// Compiles a declaration into an avatar.
///
/// The 1st pass collects every declared name, and the 2nd pass resolves references and checks types
/// while it builds the output. Every error found on the way is returned together.
pub fn transform(declaration: &decl::Avatar) -> Result<Avatar, TransformErrors> {
    let (mut context, mut errors) = Context::collect(declaration);

    let mut layers = Vec::new();
    for layer in &declaration.fx_controller {
        match layer::compile(&mut context, layer) {
            Ok(compiled) => layers.push(compiled),
            Err(error) => errors.push(error),
        }
    }
    let menu = menu::compile(&context, &declaration.menu, &mut errors);

    if !errors.is_empty() {
        return Err(TransformErrors(errors));
    }

    Ok(Avatar {
        expression_parameters: context.parameters.expression_parameters(),
        fx_controller: AnimatorController {
            parameters: context.parameters.animator_parameters(),
            layers,
        },
        menu,
        externals: context.externals,
    })
}
