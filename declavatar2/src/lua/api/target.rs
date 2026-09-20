use mlua::{Error as LuaError, FromLua, Lua, Result as LuaResult, Table, UserData, UserDataFields, UserDataMethods, Value};

use crate::{
    core::{phase::Declared, resolution::Unresolved},
    lua::{
        location::caller_location,
        node,
        value::{StrictBoolean, animated_value},
    },
    unity::{
        animation::FixedAnimationEntry,
        animator::{
            AnimatedAnimatorProperty, AnimatedAnimatorTarget, AnimatedComponentProperty, AnimatedComponentTarget, AnimatedGameObjectProperty,
            AnimatedGameObjectTarget, AnimatedRendererProperty, AnimatedRendererTarget, AnimatedTarget,
        },
        external::AssetLocator,
        value::{AnimatedValue, AnimatedValueType},
    },
};

const DEFAULT_RENDERER_TYPE: &str = "UnityEngine.SkinnedMeshRenderer";
pub(crate) const MATERIAL_TYPE: &str = "UnityEngine.Material";

pub(crate) type Reference = Unresolved<AssetLocator>;

pub(crate) fn register(lua: &Lua, da: &Table) -> LuaResult<()> {
    da.set(
        "renderer",
        lua.create_function(|lua, (path, renderer_type): (String, Option<String>)| {
            Ok(Renderer {
                path: located(lua, path),
                renderer_type: renderer_type.unwrap_or_else(|| DEFAULT_RENDERER_TYPE.into()),
            })
        })?,
    )?;
    da.set("object", lua.create_function(|lua, path: String| Ok(GameObject { path: located(lua, path) }))?)?;
    da.set(
        "component",
        lua.create_function(|lua, (path, component_type): (String, String)| {
            Ok(Component {
                path: located(lua, path),
                component_type: located(lua, component_type),
            })
        })?,
    )?;
    da.set("animator_parameter", lua.create_function(animator_parameter)?)?;
    da.set("asset", asset_table(lua)?)?;

    Ok(())
}

/// Renderer bound by `da.renderer`.
#[derive(Debug, Clone)]
pub struct Renderer {
    path: Unresolved<String>,
    renderer_type: String,
}

impl UserData for Renderer {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__name", "renderer");
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, _, ()| Ok("renderer"));

        methods.add_method("shape", |lua, this, (name, value): (String, Option<f64>)| {
            Ok(this.entry(lua, AnimatedRendererProperty::BlendShape { name }, AnimatedValue::Float(value.unwrap_or(1.0))))
        });
        methods.add_method("enabled", |lua, this, enabled: Option<StrictBoolean>| {
            Ok(this.entry(
                lua,
                AnimatedRendererProperty::Enabled,
                AnimatedValue::Bool(enabled.map(|value| value.0).unwrap_or(true)),
            ))
        });
        methods.add_method("material", |lua, this, (slot, asset): (i64, AssetArgument)| {
            let slot = material_slot("da.renderer:material", slot)?;
            Ok(this.entry(
                lua,
                AnimatedRendererProperty::Material { slot },
                AnimatedValue::ObjectReference(asset.into_reference(lua, MATERIAL_TYPE)),
            ))
        });
        methods.add_method("property", |lua, this, (name, value): (String, Value)| {
            let value = animated_value("da.renderer:property", &value)?;
            Ok(this.entry(lua, AnimatedRendererProperty::MaterialProperty { name }, value))
        });
        methods.add_method("reference", |lua, this, (name, asset): (String, node::Asset)| {
            Ok(this.entry(
                lua,
                AnimatedRendererProperty::MaterialProperty { name },
                AnimatedValue::ObjectReference(located(lua, asset.0)),
            ))
        });
    }
}

impl Renderer {
    fn entry(&self, lua: &Lua, property: AnimatedRendererProperty, value: AnimatedValue<Reference>) -> node::Target {
        target(
            AnimatedTarget::Renderer(AnimatedRendererTarget {
                path: relocated(lua, &self.path),
                renderer_type: self.renderer_type.clone(),
                property,
            }),
            value,
        )
    }
}

/// GameObject bound by `da.object`.
#[derive(Debug, Clone)]
pub struct GameObject {
    path: Unresolved<String>,
}

impl UserData for GameObject {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__name", "object");
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, _, ()| Ok("object"));

        methods.add_method("active", |lua, this, active: Option<StrictBoolean>| {
            Ok(this.entry(
                lua,
                AnimatedGameObjectProperty::Active,
                AnimatedValue::Bool(active.map(|value| value.0).unwrap_or(true)),
            ))
        });
        methods.add_method("position", |lua, this, written: Value| {
            let value = vector3("da.object:position", &written)?;
            Ok(this.entry(lua, AnimatedGameObjectProperty::TransformPosition, value))
        });
        methods.add_method("scale", |lua, this, written: Value| {
            let value = vector3("da.object:scale", &written)?;
            Ok(this.entry(lua, AnimatedGameObjectProperty::TransformScale, value))
        });
        methods.add_method("rotation", |lua, this, written: Value| {
            let value = animated_value::<Reference>("da.object:rotation", &written)?;
            match value {
                rotation @ AnimatedValue::Quaternion(_) => Ok(this.entry(lua, AnimatedGameObjectProperty::TransformRotationQuaternion, rotation)),
                euler @ AnimatedValue::Vector3(_) => Ok(this.entry(lua, AnimatedGameObjectProperty::TransformRotationEuler, euler)),
                other => Err(wrong_value("da.object:rotation", "euler angles or `da.quat`", &other)),
            }
        });
    }
}

impl GameObject {
    fn entry(&self, lua: &Lua, property: AnimatedGameObjectProperty, value: AnimatedValue<Reference>) -> node::Target {
        target(
            AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: relocated(lua, &self.path),
                property,
            }),
            value,
        )
    }
}

/// Component bound by `da.component`.
#[derive(Debug, Clone)]
pub struct Component {
    path: Unresolved<String>,
    component_type: Unresolved<String>,
}

impl UserData for Component {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__name", "component");
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, _, ()| Ok("component"));

        methods.add_method("enabled", |lua, this, enabled: Option<StrictBoolean>| {
            Ok(this.entry(
                lua,
                AnimatedComponentProperty::Enabled,
                AnimatedValue::Bool(enabled.map(|value| value.0).unwrap_or(true)),
            ))
        });
        methods.add_method("property", |lua, this, (name, written): (String, Value)| {
            let value = animated_value("da.component:property", &written)?;
            Ok(this.entry(lua, AnimatedComponentProperty::Serialized { name }, value))
        });
        methods.add_method("reference", |lua, this, (name, asset): (String, node::Asset)| {
            Ok(this.entry(
                lua,
                AnimatedComponentProperty::Serialized { name },
                AnimatedValue::ObjectReference(located(lua, asset.0)),
            ))
        });
    }
}

impl Component {
    fn entry(&self, lua: &Lua, property: AnimatedComponentProperty, value: AnimatedValue<Reference>) -> node::Target {
        target(
            AnimatedTarget::Component(AnimatedComponentTarget {
                path: relocated(lua, &self.path),
                component_type: relocated(lua, &self.component_type),
                value_type: value_type_of(&value),
                property,
            }),
            value,
        )
    }
}

fn animator_parameter(lua: &Lua, (name, value): (String, Option<f64>)) -> LuaResult<node::Target> {
    Ok(target(
        AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget {
            property: AnimatedAnimatorProperty::ParameterFloatValue { name: located(lua, name) },
        }),
        AnimatedValue::Float(value.unwrap_or(1.0)),
    ))
}

fn asset_table(lua: &Lua) -> LuaResult<Table> {
    let asset = lua.create_table()?;
    asset.set("guid", lua.create_function(|_, guid: String| Ok(node::Asset(AssetLocator::Guid(guid))))?)?;
    asset.set("path", lua.create_function(|_, path: String| Ok(node::Asset(AssetLocator::Path(path))))?)?;
    asset.set(
        "named",
        lua.create_function(|_, (asset_type, name): (String, String)| Ok(node::Asset(AssetLocator::Named { asset_type, name })))?,
    )?;
    Ok(asset)
}

/// Asset written either as a bare name or as an explicit locator.
pub(crate) enum AssetArgument {
    Named(String),
    Locator(AssetLocator),
}

impl AssetArgument {
    /// Turns the argument into a reference, giving a bare name the type its position implies.
    pub(crate) fn into_reference(self, lua: &Lua, bare_type: &str) -> Reference {
        let locator = match self {
            AssetArgument::Named(name) => AssetLocator::Named {
                asset_type: bare_type.into(),
                name,
            },
            AssetArgument::Locator(locator) => locator,
        };
        located(lua, locator)
    }
}

impl FromLua for AssetArgument {
    fn from_lua(value: Value, lua: &Lua) -> LuaResult<Self> {
        match &value {
            Value::String(name) => Ok(AssetArgument::Named(name.to_string_lossy())),
            Value::UserData(userdata) if userdata.is::<node::Asset>() => Ok(AssetArgument::Locator(node::Asset::from_lua(value.clone(), lua)?.0)),
            other => Err(LuaError::runtime(format!(
                "expected an asset name or `da.asset.*`, got {}",
                node::describe(other)
            ))),
        }
    }
}

fn target(key: AnimatedTarget<Declared>, value: AnimatedValue<Reference>) -> node::Target {
    node::Target(FixedAnimationEntry { key, value })
}

pub(crate) fn located<T>(lua: &Lua, value: T) -> Unresolved<T> {
    match caller_location(lua) {
        Some(at) => Unresolved::located(value, at),
        None => Unresolved::new(value),
    }
}

/// Rebinds a path recorded when the object was bound to the line that is using it now.
fn relocated(lua: &Lua, bound: &Unresolved<String>) -> Unresolved<String> {
    located(lua, bound.value.clone())
}

fn vector3(owner: &'static str, written: &Value) -> LuaResult<AnimatedValue<Reference>> {
    match animated_value(owner, written)? {
        vector @ AnimatedValue::Vector3(_) => Ok(vector),
        other => Err(wrong_value(owner, "three components", &other)),
    }
}

fn material_slot(owner: &'static str, written: i64) -> LuaResult<u32> {
    u32::try_from(written).map_err(|_| LuaError::runtime(format!("{owner}: a material slot must not be negative, but {written} was written")))
}

fn value_type_of(value: &AnimatedValue<Reference>) -> AnimatedValueType {
    value.value_type()
}

fn wrong_value(owner: &'static str, expected: &str, value: &AnimatedValue<Reference>) -> LuaError {
    LuaError::runtime(format!("{owner}: expected {expected}, but a {:?} value was written", value.value_type()))
}

#[cfg(test)]
mod tests {
    use nalgebra::{Vector3, Vector4};
    use rstest::*;

    use super::*;
    use crate::{
        core::resolution::SourceLocation,
        lua::testing::{eval, eval_error},
    };

    fn entry_of(expression: &str) -> FixedAnimationEntry<Declared> {
        let (lua, value) = eval(expression);
        node::Target::from_lua(value, &lua).expect("a target should be built").0
    }

    fn key_of(expression: &str) -> AnimatedTarget<Declared> {
        entry_of(expression).key
    }

    fn value_of(expression: &str) -> AnimatedValue<Reference> {
        entry_of(expression).value
    }

    fn renderer_key(path: &str, renderer_type: &str, property: AnimatedRendererProperty) -> AnimatedTarget<Declared> {
        AnimatedTarget::Renderer(AnimatedRendererTarget {
            path: Unresolved::new(path.into()),
            renderer_type: renderer_type.into(),
            property,
        })
    }

    fn reference(locator: AssetLocator) -> AnimatedValue<Reference> {
        AnimatedValue::ObjectReference(Unresolved::new(locator))
    }

    #[rstest]
    fn a_renderer_defaults_to_a_skinned_mesh_renderer() {
        assert_eq!(
            key_of("da.renderer('Face'):shape('smile')"),
            renderer_key("Face", DEFAULT_RENDERER_TYPE, AnimatedRendererProperty::BlendShape { name: "smile".into() }),
        );
        assert_eq!(
            key_of("da.renderer('Body', 'UnityEngine.MeshRenderer'):enabled()"),
            renderer_key("Body", "UnityEngine.MeshRenderer", AnimatedRendererProperty::Enabled),
        );
    }

    #[rstest]
    fn an_omitted_value_means_full() {
        assert_eq!(value_of("da.renderer('Face'):shape('smile')"), AnimatedValue::Float(1.0));
        assert_eq!(value_of("da.renderer('Face'):enabled()"), AnimatedValue::Bool(true));
        assert_eq!(value_of("da.object('Hat'):active()"), AnimatedValue::Bool(true));
        assert_eq!(value_of("da.animator_parameter('Weight')"), AnimatedValue::Float(1.0));
    }

    #[rstest]
    fn a_written_value_is_taken_as_is() {
        assert_eq!(value_of("da.renderer('Face'):shape('smile', 0.3)"), AnimatedValue::Float(0.3));
        assert_eq!(value_of("da.renderer('Face'):enabled(false)"), AnimatedValue::Bool(false));
        assert_eq!(value_of("da.object('Hat'):active(false)"), AnimatedValue::Bool(false));
        assert_eq!(value_of("da.animator_parameter('Weight', 0.25)"), AnimatedValue::Float(0.25));
    }

    #[rstest]
    fn a_bare_material_name_becomes_a_named_material_locator() {
        assert_eq!(
            key_of("da.renderer('Body'):material(2, 'Skin')"),
            renderer_key("Body", DEFAULT_RENDERER_TYPE, AnimatedRendererProperty::Material { slot: 2 }),
        );
        assert_eq!(
            value_of("da.renderer('Body'):material(2, 'Skin')"),
            reference(AssetLocator::Named {
                asset_type: MATERIAL_TYPE.into(),
                name: "Skin".into(),
            }),
        );
    }

    #[rstest]
    #[case::guid("da.asset.guid('0123abcd')", AssetLocator::Guid("0123abcd".into()))]
    #[case::path("da.asset.path('Assets/Skin.mat')", AssetLocator::Path("Assets/Skin.mat".into()))]
    #[case::named(
        "da.asset.named('UnityEngine.Texture2D', 'Eye')",
        AssetLocator::Named { asset_type: "UnityEngine.Texture2D".into(), name: "Eye".into() },
    )]
    fn an_explicit_locator_is_taken_as_written(#[case] written: &str, #[case] expected: AssetLocator) {
        assert_eq!(value_of(&format!("da.renderer('Body'):material(0, {written})")), reference(expected));
    }

    #[rstest]
    fn a_material_slot_cannot_be_negative() {
        let message = eval_error("da.renderer('Body'):material(-1, 'Skin')");
        assert!(message.contains("a material slot must not be negative"), "{message}");
    }

    #[rstest]
    fn a_renderer_property_writes_a_material_property() {
        assert_eq!(
            key_of("da.renderer('Body'):property('_Color', da.color(1, 0, 0))"),
            renderer_key(
                "Body",
                DEFAULT_RENDERER_TYPE,
                AnimatedRendererProperty::MaterialProperty { name: "_Color".into() },
            ),
        );
        assert_eq!(
            value_of("da.renderer('Body'):property('_Color', da.color(1, 0, 0))"),
            AnimatedValue::Color(Vector4::new(1.0, 0.0, 0.0, 1.0)),
        );
    }

    #[rstest]
    fn a_renderer_reference_writes_an_object_reference_material_property() {
        assert_eq!(
            key_of("da.renderer('Body'):reference('_MainTex', da.asset.guid('abc'))"),
            renderer_key(
                "Body",
                DEFAULT_RENDERER_TYPE,
                AnimatedRendererProperty::MaterialProperty { name: "_MainTex".into() },
            ),
        );
        assert_eq!(
            value_of("da.renderer('Body'):reference('_MainTex', da.asset.guid('abc'))"),
            reference(AssetLocator::Guid("abc".into())),
        );
    }

    #[rstest]
    #[case::guid("da.asset.guid('abc')", AssetLocator::Guid("abc".into()))]
    #[case::path("da.asset.path('Assets/Eye.png')", AssetLocator::Path("Assets/Eye.png".into()))]
    #[case::named("da.asset.named('UnityEngine.Texture2D', 'Eye')", AssetLocator::Named {
        asset_type: "UnityEngine.Texture2D".into(), name: "Eye".into()
    })]
    fn references_preserve_explicit_locators(
        #[case] asset: &str,
        #[case] expected: AssetLocator,
        #[values("da.renderer('Body')", "da.component('Body', 'ExampleComponent')")] receiver: &str,
    ) {
        let value = value_of(&format!("{receiver}:reference('Texture', {asset})"));
        let AnimatedValue::ObjectReference(actual) = value else {
            panic!("expected an object reference")
        };
        assert_eq!(actual.value, expected);
        assert_eq!(
            actual.at,
            Some(SourceLocation {
                chunk: "test.lua".into(),
                line: 2
            })
        );
    }

    #[rstest]
    fn references_reject_bare_names(#[values("da.renderer('Body')", "da.component('Body', 'ExampleComponent')")] receiver: &str) {
        let message = eval_error(&format!("{receiver}:reference('Texture', 'Eye')"));
        assert!(message.contains("expected asset, got string"), "{message}");
        assert!(message.contains("test.lua:2:"), "{message}");
    }

    #[rstest]
    fn boolean_targets_preserve_written_values(
        #[values("da.renderer('Body'):enabled", "da.component('Body', 'UnityEngine.Light'):enabled", "da.object('Hat'):active")] method: &str,
        #[values("true", "false", "nil", "")] value: &str,
    ) {
        assert_eq!(value_of(&format!("{method}({value})")), AnimatedValue::Bool(value != "false"));
    }

    #[rstest]
    #[case::position("position", AnimatedGameObjectProperty::TransformPosition)]
    #[case::scale("scale", AnimatedGameObjectProperty::TransformScale)]
    fn a_transform_vector_needs_three_components(#[case] method: &str, #[case] expected: AnimatedGameObjectProperty) {
        assert_eq!(
            key_of(&format!("da.object('Hat'):{method}(da.vec3(1, 2, 3))")),
            AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: Unresolved::new("Hat".into()),
                property: expected,
            }),
        );
        assert_eq!(
            value_of(&format!("da.object('Hat'):{method}({{ 1, 2, 3 }})")),
            AnimatedValue::Vector3(Vector3::new(1.0, 2.0, 3.0)),
        );

        let message = eval_error(&format!("da.object('Hat'):{method}(da.vec2(1, 2))"));
        assert!(message.contains("expected three components"), "{message}");
    }

    #[rstest]
    fn a_rotation_tells_euler_angles_from_a_quaternion() {
        assert_eq!(
            key_of("da.object('Hat'):rotation(da.vec3(0, 90, 0))"),
            AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: Unresolved::new("Hat".into()),
                property: AnimatedGameObjectProperty::TransformRotationEuler,
            }),
        );
        assert_eq!(
            key_of("da.object('Hat'):rotation(da.quat(0, 0, 0, 1))"),
            AnimatedTarget::GameObject(AnimatedGameObjectTarget {
                path: Unresolved::new("Hat".into()),
                property: AnimatedGameObjectProperty::TransformRotationQuaternion,
            }),
        );

        let message = eval_error("da.object('Hat'):rotation(da.vec4(1, 2, 3, 4))");
        assert!(message.contains("expected euler angles or `da.quat`"), "{message}");
    }

    #[rstest]
    #[case::boolean("true", AnimatedValueType::Bool)]
    #[case::integer("2", AnimatedValueType::Int)]
    #[case::float("0.5", AnimatedValueType::Float)]
    #[case::vector("da.vec3(1, 2, 3)", AnimatedValueType::Vector3)]
    #[case::color("da.color(1, 1, 1)", AnimatedValueType::Color)]
    #[case::quaternion("da.quat(0, 0, 0, 1)", AnimatedValueType::Quaternion)]
    fn a_component_property_takes_its_type_from_the_written_value(#[case] written: &str, #[case] expected: AnimatedValueType) {
        let key = key_of(&format!("da.component('Root/Bone', 'VRCPhysBone'):property('pull', {written})"));
        let AnimatedTarget::Component(component) = key else {
            panic!("expected a component target");
        };

        assert_eq!(component.component_type, Unresolved::new("VRCPhysBone".into()));
        assert_eq!(component.property, AnimatedComponentProperty::Serialized { name: "pull".into() });
        assert_eq!(component.value_type, expected);
    }

    #[rstest]
    fn a_component_reference_is_typed_as_an_object_reference() {
        let key = key_of("da.component('Root', 'UnityEngine.MeshFilter'):reference('m_Mesh', da.asset.guid('m'))");
        let AnimatedTarget::Component(component) = key else {
            panic!("expected a component target");
        };

        assert_eq!(component.value_type, AnimatedValueType::ObjectReference);
        assert_eq!(component.property, AnimatedComponentProperty::Serialized { name: "m_Mesh".into() });
    }

    #[rstest]
    fn a_component_enabled_is_a_boolean() {
        let key = key_of("da.component('Root/Light', 'UnityEngine.Light'):enabled(false)");
        let AnimatedTarget::Component(component) = key else {
            panic!("expected a component target");
        };

        assert_eq!(component.property, AnimatedComponentProperty::Enabled);
        assert_eq!(component.value_type, AnimatedValueType::Bool);
    }

    #[rstest]
    fn an_animator_parameter_animates_the_animator_itself() {
        assert_eq!(
            key_of("da.animator_parameter('Weight')"),
            AnimatedTarget::AnimatorSelf(AnimatedAnimatorTarget {
                property: AnimatedAnimatorProperty::ParameterFloatValue {
                    name: Unresolved::new("Weight".into()),
                },
            }),
        );
    }

    #[rstest]
    fn a_path_is_recorded_where_it_is_used_rather_than_where_it_was_bound() {
        let (lua, value) = eval("(function()\n  local Face = da.renderer('Face')\n  return Face:shape('smile')\nend)()");
        let entry = node::Target::from_lua(value, &lua).expect("a target should be built").0;
        let AnimatedTarget::Renderer(renderer) = entry.key else {
            panic!("expected a renderer target");
        };

        assert_eq!(
            renderer.path.at,
            Some(SourceLocation {
                chunk: "test.lua".into(),
                line: 4,
            })
        );
    }

    #[rstest]
    fn an_asset_argument_rejects_anything_else() {
        let message = eval_error("da.renderer('Body'):material(0, 42)");
        assert!(message.contains("expected an asset name or `da.asset.*`, got number"), "{message}");
    }
}
