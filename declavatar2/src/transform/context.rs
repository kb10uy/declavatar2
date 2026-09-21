use std::collections::BTreeMap;

use crate::{
    avatar::controller::ParameterRef,
    core::resolution::{Resolved, SourceLocation, Unresolved},
    decl::{
        layer::Layer,
        parameter::{Parameter, ParameterScope, PrimitiveParameter, PrimitiveParameterValue, ProvidedParameterGroup},
    },
    transform::error::{TransformError, TransformErrorKind},
    unity::{
        animator::{AnimatorParameter, AnimatorParameterTypeDefault},
        external::Externals,
        value::AnimatedValueType,
    },
    vrchat::expr_parameter::{ExpressionParameter, ExpressionParameterTypeDefault, ExpressionParameterWidth, VrchatProvidedParameter},
};

/// Everything the 1st pass collects, and what the 2nd pass resolves against.
pub(crate) struct Context {
    pub parameters: ParameterTable,
    pub layers: BTreeMap<String, LayerInfo>,
    pub externals: Externals,
}

impl Context {
    /// Runs the 1st pass: records every declared name without resolving anything.
    pub fn collect(declaration: &crate::decl::Avatar) -> (Self, Vec<TransformError>) {
        let mut context = Self {
            parameters: ParameterTable::default(),
            layers: BTreeMap::new(),
            externals: Externals::default(),
        };
        let mut errors = Vec::new();

        for parameter in &declaration.parameters {
            if let Err(error) = context.collect_parameter(parameter) {
                errors.push(error);
            }
        }
        for layer in declaration.layers() {
            if let Err(error) = context.collect_layer(layer) {
                errors.push(error);
            }
        }

        (context, errors)
    }

    fn collect_parameter(&mut self, parameter: &Parameter) -> Result<(), TransformError> {
        match parameter {
            Parameter::Primitive(primitive) => self.parameters.declare(primitive.clone()),
            Parameter::Provided(ProvidedParameterGroup::Vrchat) => {
                if self.parameters.provided_groups.contains(&ProvidedParameterGroup::Vrchat) {
                    return Err(TransformErrorKind::DuplicateProvidedGroup { group: "VRChat".into() }.into());
                }
                self.parameters.provided_groups.push(ProvidedParameterGroup::Vrchat);
                for provided in VrchatProvidedParameter::ALL {
                    self.parameters.provide(*provided)?;
                }
                Ok(())
            }
        }
    }

    fn collect_layer(&mut self, layer: &Layer) -> Result<(), TransformError> {
        let info = match layer {
            Layer::Group(group) => {
                let mut options = BTreeMap::new();
                for (offset, option) in group.options.iter().enumerate() {
                    if options.insert(option.name.clone(), offset as i64 + 1).is_some() {
                        return Err(TransformErrorKind::DuplicateOption {
                            layer: group.name.clone(),
                            option: option.name.clone(),
                        }
                        .at(option.at.clone().or_else(|| group.at.clone())));
                    }
                }
                LayerInfo::Group {
                    parameter: driver(group.driven_by.as_ref(), &group.name, group.at.as_ref()),
                    options,
                }
            }
            Layer::Switch(switch) => LayerInfo::Switch {
                parameter: driver(switch.driven_by.as_ref(), &switch.name, switch.at.as_ref()),
            },
            Layer::Puppet(puppet) => LayerInfo::Puppet {
                parameter: driver(puppet.driven_by.as_ref(), &puppet.name, puppet.at.as_ref()),
            },
            Layer::Blend(blend) => {
                self.register_layer(&blend.name, LayerInfo::Blend, blend.at.as_ref())?;
                for puppet in &blend.puppets {
                    self.register_layer(
                        &puppet.name,
                        LayerInfo::Puppet {
                            parameter: driver(puppet.driven_by.as_ref(), &puppet.name, puppet.at.as_ref()),
                        },
                        puppet.at.as_ref(),
                    )?;
                }
                return Ok(());
            }
            Layer::Raw(_) => LayerInfo::Raw,
        };
        self.register_layer(layer.name(), info, layer.at())
    }

    fn register_layer(&mut self, name: &str, info: LayerInfo, at: Option<&SourceLocation>) -> Result<(), TransformError> {
        if self.layers.contains_key(name) {
            return Err(TransformErrorKind::DuplicateLayer { name: name.to_owned() }.at(at.cloned()));
        }
        self.layers.insert(name.to_owned(), info);
        Ok(())
    }

    pub fn layer(&self, reference: &Unresolved<String>) -> Result<&LayerInfo, TransformError> {
        self.layers
            .get(&reference.value)
            .ok_or_else(|| TransformErrorKind::UnknownLayer { name: reference.value.clone() }.at(reference.at.clone()))
    }
}

/// The parameter a layer follows: what `driven_by` names, or the layer's own name when it is omitted.
pub(crate) fn driver(written: Option<&Unresolved<String>>, layer_name: &str, at: Option<&SourceLocation>) -> Unresolved<String> {
    match written {
        Some(parameter) => parameter.clone(),
        None => Unresolved {
            value: layer_name.to_owned(),
            at: at.cloned(),
        },
    }
}

/// What the 1st pass knows about a layer: enough to resolve `da.drive_*` against it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LayerInfo {
    Group {
        parameter: Unresolved<String>,
        options: BTreeMap<String, i64>,
    },
    Switch {
        parameter: Unresolved<String>,
    },
    Puppet {
        parameter: Unresolved<String>,
    },
    Blend,
    Raw,
}

impl LayerInfo {
    pub fn kind(&self) -> &'static str {
        match self {
            LayerInfo::Group { .. } => "group",
            LayerInfo::Switch { .. } => "switch",
            LayerInfo::Puppet { .. } => "puppet",
            LayerInfo::Blend => "blend",
            LayerInfo::Raw => "raw",
        }
    }
}

/// Every animator parameter of the avatar, in the order they were declared or generated.
#[derive(Debug, Default)]
pub(crate) struct ParameterTable {
    entries: Vec<ParameterEntry>,
    index: BTreeMap<String, usize>,
    provided_groups: Vec<ProvidedParameterGroup>,
}

#[derive(Debug, Clone, PartialEq)]
struct ParameterEntry {
    name: String,
    type_default: AnimatorParameterTypeDefault,
    source: ParameterSource,
}

#[derive(Debug, Clone, PartialEq)]
enum ParameterSource {
    Declared(PrimitiveParameter),
    Provided(VrchatProvidedParameter),
    Generated,
}

impl ParameterTable {
    fn declare(&mut self, parameter: PrimitiveParameter) -> Result<(), TransformError> {
        if let Some(existing) = self.index.get(&parameter.name).map(|&index| &self.entries[index]) {
            let kind = match existing.source {
                ParameterSource::Provided(_) => TransformErrorKind::ParameterCollidesWithProvided { name: parameter.name.clone() },
                ParameterSource::Declared(_) => TransformErrorKind::DuplicateParameter { name: parameter.name.clone() },
                ParameterSource::Generated => TransformErrorKind::GeneratedParameterCollision { name: parameter.name.clone() },
            };
            return Err(kind.at(parameter.at.clone()));
        }
        let type_default = match parameter.value {
            PrimitiveParameterValue::Bool { default } => AnimatorParameterTypeDefault::Bool(default),
            PrimitiveParameterValue::Int { default, .. } => AnimatorParameterTypeDefault::Int(
                default
                    .map(|value| {
                        i32::try_from(value).map_err(|_| {
                            TransformErrorKind::ParameterDefaultOutOfRange {
                                name: parameter.name.clone(),
                                value,
                            }
                            .at(parameter.at.clone())
                        })
                    })
                    .transpose()?,
            ),
            PrimitiveParameterValue::Float { default, .. } => AnimatorParameterTypeDefault::Float(default.map(|value| value as f32)),
        };
        self.push(ParameterEntry {
            name: parameter.name.clone(),
            type_default,
            source: ParameterSource::Declared(parameter),
        });
        Ok(())
    }

    fn provide(&mut self, provided: VrchatProvidedParameter) -> Result<(), TransformError> {
        if let Some(existing) = self.index.get(provided.name()).map(|&index| &self.entries[index]) {
            let at = match &existing.source {
                ParameterSource::Declared(declared) => declared.at.clone(),
                _ => None,
            };
            return Err(TransformErrorKind::ParameterCollidesWithProvided { name: provided.name().into() }.at(at));
        }
        self.push(ParameterEntry {
            name: provided.name().into(),
            type_default: match provided.animated_value_type() {
                AnimatedValueType::Bool => AnimatorParameterTypeDefault::Bool(None),
                AnimatedValueType::Int => AnimatorParameterTypeDefault::Int(None),
                AnimatedValueType::Float => AnimatorParameterTypeDefault::Float(None),
                _ => unreachable!("provided parameters hold a bool, an int or a float"),
            },
            source: ParameterSource::Provided(provided),
        });
        Ok(())
    }

    /// Adds an animator-only parameter the transform needs for itself.
    pub fn generate(&mut self, name: String, type_default: AnimatorParameterTypeDefault) -> Result<ParameterRef, TransformError> {
        if self.index.contains_key(&name) {
            return Err(TransformErrorKind::GeneratedParameterCollision { name }.into());
        }
        let value_type = type_default.value_type().animated_value_type();
        self.push(ParameterEntry {
            name: name.clone(),
            type_default,
            source: ParameterSource::Generated,
        });
        Ok(Resolved::new(name, value_type))
    }

    fn push(&mut self, entry: ParameterEntry) {
        self.index.insert(entry.name.clone(), self.entries.len());
        self.entries.push(entry);
    }

    pub fn resolve(&self, reference: &Unresolved<String>) -> Result<ParameterRef, TransformError> {
        let entry = self
            .index
            .get(&reference.value)
            .map(|&index| &self.entries[index])
            .ok_or_else(|| TransformErrorKind::UnknownParameter { name: reference.value.clone() }.at(reference.at.clone()))?;
        Ok(Resolved::new(entry.name.clone(), entry.type_default.value_type().animated_value_type()))
    }

    pub fn resolve_typed(&self, reference: &Unresolved<String>, expected: AnimatedValueType) -> Result<ParameterRef, TransformError> {
        let resolved = self.resolve(reference)?;
        if resolved.context != expected {
            return Err(TransformErrorKind::ParameterTypeMismatch {
                name: resolved.value,
                expected,
                found: resolved.context,
            }
            .at(reference.at.clone()));
        }
        Ok(resolved)
    }

    pub fn animator_parameters(&self) -> Vec<AnimatorParameter> {
        self.entries
            .iter()
            .map(|entry| AnimatorParameter {
                name: entry.name.clone(),
                type_default: entry.type_default,
            })
            .collect()
    }

    pub fn expression_parameters(&self) -> Vec<ExpressionParameter> {
        self.entries
            .iter()
            .filter_map(|entry| match &entry.source {
                ParameterSource::Declared(declared) => expression_parameter(declared, entry.type_default),
                _ => None,
            })
            .collect()
    }
}

fn expression_parameter(declared: &PrimitiveParameter, compiled: AnimatorParameterTypeDefault) -> Option<ExpressionParameter> {
    let scope = declared.scope.unwrap_or(ParameterScope::Synced);
    if scope == ParameterScope::Internal {
        return None;
    }
    let width = match declared.value {
        PrimitiveParameterValue::Bool { .. } => None,
        PrimitiveParameterValue::Int { width, .. } | PrimitiveParameterValue::Float { width, .. } => width,
    }
    .map_or(ExpressionParameterWidth::Unspecified, ExpressionParameterWidth::Specified);
    let type_default = match compiled {
        AnimatorParameterTypeDefault::Bool(default) => ExpressionParameterTypeDefault::Bool(default),
        AnimatorParameterTypeDefault::Int(default) => ExpressionParameterTypeDefault::Int { width, default },
        AnimatorParameterTypeDefault::Float(default) => ExpressionParameterTypeDefault::Float { width, default },
    };
    Some(ExpressionParameter {
        name: declared.name.clone(),
        type_default,
        saved: declared.save.unwrap_or(false),
        synced: scope == ParameterScope::Synced,
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        decl::{
            Avatar,
            behavior::Content,
            controller::Controller,
            layer::{BlendLayer, GroupLayer, GroupOption, PuppetLayer, SwitchContent, SwitchLayer},
        },
        vrchat::playable_layer::PlayableLayer,
    };

    fn at(line: u32) -> Option<SourceLocation> {
        Some(SourceLocation {
            chunk: "avatar.lua".into(),
            line,
        })
    }

    fn primitive(name: &str, value: PrimitiveParameterValue, line: u32) -> Parameter {
        Parameter::Primitive(PrimitiveParameter {
            name: name.into(),
            value,
            scope: None,
            save: None,
            at: at(line),
        })
    }

    fn int(name: &str, line: u32) -> Parameter {
        primitive(name, PrimitiveParameterValue::Int { default: None, width: None }, line)
    }

    fn collect(declaration: Avatar) -> (Context, Vec<TransformError>) {
        Context::collect(&declaration)
    }

    #[rstest]
    fn parameters_are_recorded_with_their_types() {
        let (context, errors) = collect(Avatar {
            parameters: vec![
                int("Emote", 1),
                primitive("Hat", PrimitiveParameterValue::Bool { default: Some(true) }, 2),
                primitive(
                    "Wink",
                    PrimitiveParameterValue::Float {
                        default: Some(0.5),
                        width: Some(4),
                    },
                    3,
                ),
            ],
            ..Avatar::default()
        });

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(context.parameters.resolve(&"Emote".to_owned().into()).unwrap().context, AnimatedValueType::Int);
        assert_eq!(context.parameters.resolve(&"Hat".to_owned().into()).unwrap().context, AnimatedValueType::Bool);
        assert_eq!(context.parameters.resolve(&"Wink".to_owned().into()).unwrap().context, AnimatedValueType::Float);
        assert_eq!(
            context.parameters.animator_parameters(),
            vec![
                AnimatorParameter::create_int("Emote", None),
                AnimatorParameter::create_bool("Hat", Some(true)),
                AnimatorParameter::create_float("Wink", Some(0.5)),
            ]
        );
    }

    #[rstest]
    fn a_duplicate_parameter_is_reported_at_the_second_declaration() {
        let (_, errors) = collect(Avatar {
            parameters: vec![int("Emote", 1), int("Emote", 2)],
            ..Avatar::default()
        });

        assert_eq!(errors, vec![TransformErrorKind::DuplicateParameter { name: "Emote".into() }.at(at(2))]);
    }

    #[rstest]
    fn provided_parameters_are_declared_in_bulk() {
        let (context, errors) = collect(Avatar {
            parameters: vec![Parameter::Provided(ProvidedParameterGroup::Vrchat)],
            ..Avatar::default()
        });

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            context.parameters.resolve(&"GestureLeft".to_owned().into()).unwrap().context,
            AnimatedValueType::Int
        );
        assert_eq!(context.parameters.resolve(&"AFK".to_owned().into()).unwrap().context, AnimatedValueType::Bool);
        assert!(context.parameters.expression_parameters().is_empty());
        assert_eq!(context.parameters.animator_parameters().len(), VrchatProvidedParameter::ALL.len());
    }

    #[rstest]
    #[case::declared_first(vec![int("GestureLeft", 1), Parameter::Provided(ProvidedParameterGroup::Vrchat)])]
    #[case::provided_first(vec![Parameter::Provided(ProvidedParameterGroup::Vrchat), int("GestureLeft", 1)])]
    fn a_parameter_may_not_shadow_a_provided_one(#[case] parameters: Vec<Parameter>) {
        let (_, errors) = collect(Avatar {
            parameters,
            ..Avatar::default()
        });

        assert_eq!(
            errors,
            vec![TransformErrorKind::ParameterCollidesWithProvided { name: "GestureLeft".into() }.at(at(1))]
        );
    }

    #[rstest]
    fn a_provided_group_is_declared_once() {
        let (_, errors) = collect(Avatar {
            parameters: vec![
                Parameter::Provided(ProvidedParameterGroup::Vrchat),
                Parameter::Provided(ProvidedParameterGroup::Vrchat),
            ],
            ..Avatar::default()
        });

        assert_eq!(errors, vec![TransformErrorKind::DuplicateProvidedGroup { group: "VRChat".into() }.into()]);
    }

    #[rstest]
    fn an_unknown_parameter_is_reported_where_it_was_referenced() {
        let (context, _) = collect(Avatar::default());
        let error = context
            .parameters
            .resolve(&Unresolved::located("Emote".to_owned(), at(4).unwrap()))
            .unwrap_err();
        assert_eq!(error, TransformErrorKind::UnknownParameter { name: "Emote".into() }.at(at(4)));
    }

    #[rstest]
    fn a_typed_lookup_rejects_the_wrong_type() {
        let (context, _) = collect(Avatar {
            parameters: vec![int("Emote", 1)],
            ..Avatar::default()
        });
        let error = context
            .parameters
            .resolve_typed(&Unresolved::located("Emote".to_owned(), at(4).unwrap()), AnimatedValueType::Bool)
            .unwrap_err();
        assert_eq!(
            error,
            TransformErrorKind::ParameterTypeMismatch {
                name: "Emote".into(),
                expected: AnimatedValueType::Bool,
                found: AnimatedValueType::Int,
            }
            .at(at(4))
        );
    }

    #[rstest]
    fn expression_parameters_follow_scope_and_save() {
        let (context, _) = collect(Avatar {
            parameters: vec![
                Parameter::Primitive(PrimitiveParameter {
                    name: "Synced".into(),
                    value: PrimitiveParameterValue::Int {
                        default: Some(3),
                        width: Some(4),
                    },
                    scope: None,
                    save: Some(true),
                    at: None,
                }),
                Parameter::Primitive(PrimitiveParameter {
                    name: "Local".into(),
                    value: PrimitiveParameterValue::Bool { default: None },
                    scope: Some(ParameterScope::Local),
                    save: None,
                    at: None,
                }),
                Parameter::Primitive(PrimitiveParameter {
                    name: "Internal".into(),
                    value: PrimitiveParameterValue::Float {
                        default: Some(1.0),
                        width: None,
                    },
                    scope: Some(ParameterScope::Internal),
                    save: None,
                    at: None,
                }),
            ],
            ..Avatar::default()
        });

        assert_eq!(
            context.parameters.expression_parameters(),
            vec![
                ExpressionParameter {
                    name: "Synced".into(),
                    type_default: ExpressionParameterTypeDefault::Int {
                        width: ExpressionParameterWidth::Specified(4),
                        default: Some(3),
                    },
                    saved: true,
                    synced: true,
                },
                ExpressionParameter {
                    name: "Local".into(),
                    type_default: ExpressionParameterTypeDefault::Bool(None),
                    saved: false,
                    synced: false,
                },
            ]
        );
        assert_eq!(context.parameters.animator_parameters().len(), 3);
    }

    fn group(name: &str, driven_by: Option<&str>, options: &[&str], line: u32) -> Layer {
        Layer::Group(GroupLayer {
            name: name.into(),
            driven_by: driven_by.map(|parameter| parameter.to_owned().into()),
            symmetric: None,
            default: None,
            options: options
                .iter()
                .map(|option| GroupOption {
                    name: (*option).into(),
                    content: Content::new(),
                    at: None,
                })
                .collect(),
            at: at(line),
        })
    }

    fn puppet(name: &str, line: u32) -> PuppetLayer {
        PuppetLayer {
            name: name.into(),
            driven_by: None,
            keyframes: vec![],
            at: at(line),
        }
    }

    #[rstest]
    fn layers_are_recorded_with_what_drives_them() {
        let (context, errors) = collect(Avatar {
            controllers: vec![Controller::new(
                PlayableLayer::Fx,
                vec![
                    group("Expressions", Some("Emote"), &["smile", "angry"], 10),
                    Layer::Switch(SwitchLayer {
                        name: "Hat".into(),
                        driven_by: None,
                        content: SwitchContent::Toggle(Content::new()),
                        at: at(11),
                    }),
                    Layer::Blend(BlendLayer {
                        name: "Face".into(),
                        puppets: vec![puppet("Wink", 13), puppet("Brow", 14)],
                        at: at(12),
                    }),
                ],
            )],
            ..Avatar::default()
        });

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            context.layers.get("Expressions"),
            Some(&LayerInfo::Group {
                parameter: "Emote".to_owned().into(),
                options: BTreeMap::from([("smile".into(), 1), ("angry".into(), 2)]),
            })
        );
        assert_eq!(
            context.layers.get("Hat"),
            Some(&LayerInfo::Switch {
                parameter: Unresolved::located("Hat".into(), at(11).unwrap()),
            })
        );
        assert_eq!(context.layers.get("Face"), Some(&LayerInfo::Blend));
        assert_eq!(
            context.layers.get("Wink"),
            Some(&LayerInfo::Puppet {
                parameter: Unresolved::located("Wink".into(), at(13).unwrap()),
            })
        );
        assert!(context.layers.contains_key("Brow"));
    }

    #[rstest]
    fn a_duplicate_layer_is_reported_at_the_second_declaration() {
        let (_, errors) = collect(Avatar {
            controllers: vec![Controller::new(PlayableLayer::Fx, vec![group("A", None, &[], 1), group("A", None, &[], 2)])],
            ..Avatar::default()
        });

        assert_eq!(errors, vec![TransformErrorKind::DuplicateLayer { name: "A".into() }.at(at(2))]);
    }

    #[rstest]
    fn a_duplicate_option_is_reported() {
        let (_, errors) = collect(Avatar {
            controllers: vec![Controller::new(PlayableLayer::Fx, vec![group("A", None, &["x", "x"], 1)])],
            ..Avatar::default()
        });

        assert_eq!(
            errors,
            vec![
                TransformErrorKind::DuplicateOption {
                    layer: "A".into(),
                    option: "x".into(),
                }
                .at(at(1))
            ]
        );
    }
}
