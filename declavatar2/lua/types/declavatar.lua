---@meta declavatar

--- Declarative avatar description.
---
--- A script binds this module, builds one avatar and returns it:
--- ```lua
--- local da = require "declavatar"
--- return da.avatar({ ... })
--- ```
---
--- Every builder returns an opaque node. Nodes have no readable fields; they are only
--- passed on to the builder that consumes them.
local da = {}

--------------------------------------------------------------------------------
-- Nodes
--------------------------------------------------------------------------------

--- Whole avatar, what a script returns.
---@class da.Avatar

--- Entry of the `parameters` block.
---@class da.Parameter

--- Entry of the `fx_controller` block.
---@class da.Layer

--- Default state of a group layer.
---@class da.GroupDefault

--- One option of a group layer.
---@class da.GroupOption

--- One keyframe of a puppet layer.
---@class da.Keyframe

--- Entry of the `exports` block.
---@class da.Export

--- Parameter drive, which a state runs and a menu item triggers.
---@class da.Drive

--- State behavior other than a drive.
---@class da.Behavior

--- Animated target together with the value written for it.
---@class da.Target

--- Explicit locator of a Unity asset.
---@class da.Asset

--- Vector of two, three or four components.
---@class da.Vector

--- Color with an alpha channel.
---@class da.Color

--- Rotation written as a quaternion.
---@class da.Quaternion

--- Entry of the `menu` block.
---@class da.MenuItem

--- One axis of a puppet menu item.
---@class da.Axis

--- State of a raw layer.
---@class da.State

--- Transition between two states of a raw layer.
---@class da.Transition

--- What a raw state plays.
---@class da.Motion

--- Field of a blend tree placed on its axes.
---@class da.Field

--- Field of a direct blend tree, weighted by its own parameter.
---@class da.WeightedField

--- Condition of a raw transition.
---@class da.Condition

--------------------------------------------------------------------------------
-- Values written in place of a node
--------------------------------------------------------------------------------

--- How far a parameter is visible.
---@alias da.Scope "synced"|"local"|"internal"

--- Set of parameters that the platform defines.
---@alias da.ProvidedGroup "VRChat"

--- What tracking control does to the parts it names.
---@alias da.TrackingMode "tracking"|"animation"

--- Part of the body that tracking control acts on.
---@alias da.TrackingTarget "head"|"left_hand"|"right_hand"|"hip"|"left_foot"|"right_foot"|"left_fingers"|"right_fingers"|"eyes"|"mouth"

--- How a blend tree blends its fields. A `direct` tree weights each field by its own parameter.
---@alias da.BlendTreeType "linear"|"simple_2d"|"freeform_2d"|"cartesian_2d"|"direct"

--- Value written for an animated property. A plain table of two to four numbers is a vector.
---@alias da.Value boolean|number|da.Vector|da.Color|da.Quaternion|number[]

--- Vector of three components, written with `da.vec3` or as a plain table.
---@alias da.Vector3Value da.Vector|[number, number, number]

--- Rotation, written as euler angles or as a quaternion.
---@alias da.RotationValue da.Vector|da.Quaternion|[number, number, number]

--- Asset, written as a bare name or as an explicit locator.
---@alias da.AssetValue string|da.Asset

--- Axis of a puppet item, written as a parameter name, a puppet drive or `da.axis`.
---@alias da.AxisValue string|da.Drive|da.Axis

--- State of a raw layer, written as its name or as the state itself.
---@alias da.StateValue string|da.State

--- Place of a blend tree field: one number on a single axis, or two on a pair of them.
---@alias da.Position number|da.Vector|[number, number]

--------------------------------------------------------------------------------
-- Child lists
--------------------------------------------------------------------------------
-- `false` is dropped from every child list, so `cond and da.bool("X")` reads as a
-- conditional element. Nested lists are an error; use `da.flatten` instead.

---@alias da.ParameterList (da.Parameter|false)[]
---@alias da.LayerList (da.Layer|false)[]
---@alias da.MenuItemList (da.MenuItem|false)[]
---@alias da.ExportList (da.Export|false)[]
---@alias da.ContentList (da.Target|da.Drive|da.Behavior|false)[]
---@alias da.TargetList (da.Target|false)[]
---@alias da.BehaviorList (da.Drive|da.Behavior|false)[]
---@alias da.GroupChildList (da.GroupDefault|da.GroupOption|false)[]
---@alias da.KeyframeList (da.Keyframe|false)[]
---@alias da.RawChildList (da.State|da.Transition|false)[]
---@alias da.TransitionList (da.Transition|false)[]
---@alias da.ConditionList (da.Condition|false)[]
---@alias da.FieldList (da.Field|false)[]
---@alias da.WeightedFieldList (da.WeightedField|false)[]
---@alias da.TrackingTargetList (da.TrackingTarget|false)[]

--------------------------------------------------------------------------------
-- Options tables
--------------------------------------------------------------------------------
-- A key an options table does not know is an error, so a typo fails at the call site.

--- Blocks of an avatar.
---@class da.AvatarBlocks
---@field parameters? da.ParameterList
---@field fx_controller? da.LayerList
---@field menu? da.MenuItemList
---@field exports? da.ExportList

---@class da.BoolOptions
---@field default? boolean
---@field scope? da.Scope
---@field save? boolean

---@class da.IntOptions
---@field default? integer Must be written as an integer; `1.5` is an error.
---@field width? integer Positive bit count.
---@field scope? da.Scope
---@field save? boolean

---@class da.FloatOptions
---@field default? number An integer is accepted and converted.
---@field width? integer Positive bit count.
---@field scope? da.Scope
---@field save? boolean

---@class da.GroupLayerOptions
---@field driven_by? string
---@field symmetric? boolean Defaults to true, where switching between options never passes through the default state.

--- A switch layer follows either `driven_by` or `gate`, not both.
---@class da.SwitchLayerOptions
---@field driven_by? string
---@field gate? string

---@class da.PuppetLayerOptions
---@field driven_by? string Must be a float.

---@class da.RawLayerOptions
---@field default? da.StateValue

---@class da.RawStateOptions
---@field motion? da.Motion
---@field behaviors? da.BehaviorList

---@class da.TransitionOptions
---@field duration? number

---@class da.ClipOptions
---@field speed? number
---@field speed_by? string
---@field time_by? string

--- A parametric tree blends along `x`, and a two dimensional one along `y` as well.
--- A `direct` tree has neither.
---@class da.BlendTreeOptions
---@field type da.BlendTreeType
---@field x? string
---@field y? string

---@class da.AxisOptions
---@field positive? string
---@field negative? string

---@class da.TwoAxisOptions
---@field horizontal da.AxisValue
---@field vertical da.AxisValue

---@class da.FourAxisOptions
---@field up da.AxisValue
---@field down da.AxisValue
---@field left da.AxisValue
---@field right da.AxisValue

--------------------------------------------------------------------------------
-- Script wide helpers
--------------------------------------------------------------------------------

--- The avatar a script returns. A declaration carries no name of its own.
---@param blocks? da.AvatarBlocks
---@return da.Avatar
function da.avatar(blocks) end

--- Whether the client supplied that symbol. Use ordinary Lua control flow with it.
---@param name string
---@return boolean
function da.symbol(name) end

--- Expands one level of lists and drops `false`, so that groups of nodes can be spliced together.
---@param ... any
---@return any[]
function da.flatten(...) end

--- Applies `fn` to each entry of `list`, passing the entry and its one based index.
---@generic T
---@param list T[]
---@param fn fun(value: T, index: integer): any
---@return any[]
function da.map(list, fn) end

--------------------------------------------------------------------------------
-- Values
--------------------------------------------------------------------------------

---@param x number
---@param y number
---@return da.Vector
function da.vec2(x, y) end

---@param x number
---@param y number
---@param z number
---@return da.Vector
function da.vec3(x, y, z) end

---@param x number
---@param y number
---@param z number
---@param w number
---@return da.Vector
function da.vec4(x, y, z, w) end

--- A bare table never becomes a color, so this is the only way to write one.
---@param r number
---@param g number
---@param b number
---@param a? number Defaults to 1.
---@return da.Color
function da.color(r, g, b, a) end

--- A bare table never becomes a quaternion, so this is the only way to write one.
--- The components are normalized.
---@param x number
---@param y number
---@param z number
---@param w number
---@return da.Quaternion
function da.quat(x, y, z, w) end

--------------------------------------------------------------------------------
-- Parameters
--------------------------------------------------------------------------------

---@param name string
---@param options? da.BoolOptions
---@return da.Parameter
function da.bool(name, options) end

---@param name string
---@param options? da.IntOptions
---@return da.Parameter
function da.int(name, options) end

---@param name string
---@param options? da.FloatOptions
---@return da.Parameter
function da.float(name, options) end

--- Declares every parameter the platform provides. The group name matches exactly.
---@param group da.ProvidedGroup
---@return da.Parameter
function da.provided(group) end

--------------------------------------------------------------------------------
-- Animated targets
--------------------------------------------------------------------------------

--- Renderer bound to a path, from which targets are built.
---@class da.Renderer
local Renderer = {}

--- Blend shape value. An omitted value means full.
---@param name string
---@param value? number
---@return da.Target
function Renderer:shape(name, value) end

--- Enabled state of the renderer. An omitted value means enabled.
---@param enabled? boolean
---@return da.Target
function Renderer:enabled(enabled) end

--- Material of one slot. A bare name is looked up as a `UnityEngine.Material`.
---@param slot integer
---@param asset da.AssetValue
---@return da.Target
function Renderer:material(slot, asset) end

--- Material property such as `_Color`. A serialized field of the renderer itself
--- is written through `da.component(path, "UnityEngine.SkinnedMeshRenderer"):property(...)`.
---@param name string
---@param value da.Value
---@return da.Target
function Renderer:property(name, value) end

--- Material property that holds an object reference, such as a texture.
---@param name string
---@param asset da.Asset
---@return da.Target
function Renderer:reference(name, asset) end

--- GameObject bound to a path, from which targets are built.
---@class da.Object
local Object = {}

--- Active state. An omitted value means active.
---@param active? boolean
---@return da.Target
function Object:active(active) end

---@param position da.Vector3Value
---@return da.Target
function Object:position(position) end

--- Local rotation. A vector is read as euler angles and `da.quat` as a quaternion.
---@param rotation da.RotationValue
---@return da.Target
function Object:rotation(rotation) end

---@param scale da.Vector3Value
---@return da.Target
function Object:scale(scale) end

--- Component bound to a path and a type, from which targets are built.
---@class da.Component
local Component = {}

--- Enabled state of the component. An omitted value means enabled.
---@param enabled? boolean
---@return da.Target
function Component:enabled(enabled) end

--- Serialized field. Its type follows the value written for it.
---@param name string
---@param value da.Value
---@return da.Target
function Component:property(name, value) end

--- Serialized field that holds an object reference.
---@param name string
---@param asset da.Asset
---@return da.Target
function Component:reference(name, asset) end

--- Binds a renderer. The type defaults to `UnityEngine.SkinnedMeshRenderer`.
---@param path string Path relative to the avatar root.
---@param renderer_type? string
---@return da.Renderer
function da.renderer(path, renderer_type) end

--- Binds a GameObject.
---@param path string Path relative to the avatar root.
---@return da.Object
function da.object(path) end

--- Binds a component by its fully qualified type name.
---@param path string Path relative to the avatar root.
---@param component_type string
---@return da.Component
function da.component(path, component_type) end

--- Animator parameter driven by an animation, for animated animator parameters.
--- An omitted value means 1.
---@param name string
---@param value? number
---@return da.Target
function da.animator_parameter(name, value) end

--- Explicit asset locators. A bare name given to `:material` or `da.raw.external`
--- is looked up by the type its position implies.
da.asset = {}

---@param guid string
---@return da.Asset
function da.asset.guid(guid) end

---@param path string Path from the project root.
---@return da.Asset
function da.asset.path(path) end

---@param asset_type string Fully qualified type name.
---@param name string
---@return da.Asset
function da.asset.named(asset_type, name) end

--------------------------------------------------------------------------------
-- State behaviors
--------------------------------------------------------------------------------

--- Sets the layer to one of its options.
---@param layer string
---@param option string
---@return da.Drive
function da.drive_group(layer, option) end

--- Sets the layer to one of its two states. An omitted value means enabled.
---@param layer string
---@param value? boolean
---@return da.Drive
function da.drive_switch(layer, value) end

--- Sets the parameter of a puppet layer.
---@param layer string
---@param value? number
---@return da.Drive
function da.drive_puppet(layer, value) end

---@param parameter string
---@param value boolean
---@return da.Drive
function da.drive_bool(parameter, value) end

---@param parameter string
---@param value integer
---@return da.Drive
function da.drive_int(parameter, value) end

---@param parameter string
---@param value number
---@return da.Drive
function da.drive_float(parameter, value) end

--- Hands the named parts over to animation, or back to tracking.
---@param mode da.TrackingMode
---@param targets da.TrackingTargetList
---@return da.Behavior
function da.tracking(mode, targets) end

--------------------------------------------------------------------------------
-- Layers
--------------------------------------------------------------------------------

--- Default state of a group layer, which every option inherits the entries it lacks from.
---@param content da.ContentList
---@return da.GroupDefault
function da.default(content) end

--- One option of a group layer. Its index is assigned while compiling.
---@param name string
---@param content da.ContentList
---@return da.GroupOption
function da.option(name, content) end

--- Layer that switches between mutually exclusive options.
---@param name string
---@param options da.GroupLayerOptions
---@param children da.GroupChildList `da.default` at most once, then `da.option` in order.
---@return da.Layer
---@overload fun(name: string, children: da.GroupChildList): da.Layer
function da.group_layer(name, options, children) end

--- Layer that has exactly two states.
---
--- With three arguments the last list is a toggle list: it spells out the enabled side
--- and the disabled side gets the zeroed values. With four arguments both sides are
--- written out, in `disabled, enabled` order.
---@param name string
---@param options da.SwitchLayerOptions Required, so the two forms are told apart by argument count alone.
---@param disabled da.ContentList
---@param enabled da.ContentList
---@return da.Layer
---@overload fun(name: string, options: da.SwitchLayerOptions, enabled: da.ContentList): da.Layer
function da.switch_layer(name, options, disabled, enabled) end

--- One keyframe of a puppet layer. `time` is a value of the driving parameter, not normalized time.
---@param time number
---@param targets da.TargetList
---@return da.Keyframe
function da.keyframe(time, targets) end

--- Layer that interpolates its targets along a float parameter.
---@param name string
---@param options da.PuppetLayerOptions
---@param keyframes da.KeyframeList
---@return da.Layer
---@overload fun(name: string, keyframes: da.KeyframeList): da.Layer
function da.puppet_layer(name, options, keyframes) end

--- Merges its children into one layer whose single state is a direct blend tree.
--- Only puppet layers can be merged, and their targets sum instead of overriding.
---@param name string
---@param children da.LayerList
---@return da.Layer
function da.blend_layer(name, children) end

--- Declares a gate that other assets can drive.
---@param name string
---@return da.Export
function da.gate(name) end

--- Binds a gate to a parameter.
---@param gate string
---@param parameter string
---@return da.Export
function da.guard(gate, parameter) end

--------------------------------------------------------------------------------
-- Menu
--------------------------------------------------------------------------------

---@param name string
---@param items da.MenuItemList
---@return da.MenuItem
function da.submenu(name, items) end

---@param name string
---@param drive da.Drive
---@return da.MenuItem
function da.toggle(name, drive) end

---@param name string
---@param drive da.Drive
---@return da.MenuItem
function da.button(name, drive) end

---@param name string
---@param axis da.AxisValue
---@return da.MenuItem
function da.radial(name, axis) end

--- Two axis puppet. Both axes are named rather than ordered.
---@param name string
---@param axes da.TwoAxisOptions
---@return da.MenuItem
function da.two_axis(name, axes) end

--- Four axis puppet. Every direction is named rather than ordered.
---@param name string
---@param axes da.FourAxisOptions
---@return da.MenuItem
function da.four_axis(name, axes) end

--- Axis with a label on one or both of its ends.
---@param target da.AxisValue
---@param labels? da.AxisOptions
---@return da.Axis
function da.axis(target, labels) end

--------------------------------------------------------------------------------
-- Raw layers
--------------------------------------------------------------------------------

--- Layers written as a state machine, for what the other layer kinds do not reach.
da.raw = {}

--- Layer written as a state machine. A transition written here names both of its ends.
---@param name string
---@param options da.RawLayerOptions
---@param children da.RawChildList
---@return da.Layer
---@overload fun(name: string, children: da.RawChildList): da.Layer
function da.raw.layer(name, options, children) end

--- One state of a raw layer. A transition written in `outgoing` leaves this state.
---@param name string
---@param options? da.RawStateOptions
---@param outgoing? da.TransitionList
---@return da.State
function da.raw.state(name, options, outgoing) end

--- Transition between two states.
---
--- Inside a state the source is implied, so `from` is left out. A table in the second
--- place is the options table, which is how the three argument forms are told apart.
---@param from da.StateValue
---@param to da.StateValue
---@param options da.TransitionOptions
---@param conditions da.ConditionList
---@return da.Transition
---@overload fun(to: da.StateValue, conditions: da.ConditionList): da.Transition
---@overload fun(from: da.StateValue, to: da.StateValue, conditions: da.ConditionList): da.Transition
---@overload fun(to: da.StateValue, options: da.TransitionOptions, conditions: da.ConditionList): da.Transition
function da.raw.transition(from, to, options, conditions) end

--- Clip generated from the written targets.
---@param options da.ClipOptions
---@param targets da.TargetList
---@return da.Motion
---@overload fun(targets: da.TargetList): da.Motion
function da.raw.clip(options, targets) end

--- Clip that already exists as a Unity asset. A bare name is looked up as a `UnityEngine.AnimationClip`.
---@param asset da.AssetValue
---@param options? da.ClipOptions
---@return da.Motion
function da.raw.external(asset, options) end

--- Motion that blends its fields. A `direct` tree takes weighted fields and the others take placed ones.
---@param options da.BlendTreeOptions
---@param fields da.FieldList|da.WeightedFieldList
---@return da.Motion
function da.raw.blend_tree(options, fields) end

--- Field of a parametric blend tree, placed on its axes.
---@param position da.Position
---@param motion da.Motion
---@return da.Field
function da.raw.field(position, motion) end

--- Field of a direct blend tree, weighted by its own parameter.
---@param parameter string
---@param motion da.Motion
---@return da.WeightedField
function da.raw.weighted(parameter, motion) end

--- Conditions of a raw transition. The comparison type comes from the parameter type
--- while compiling, so `eq` on a float is an error.
da.raw.cond = {}

---@param parameter string
---@return da.Condition
function da.raw.cond.zero(parameter) end

---@param parameter string
---@return da.Condition
function da.raw.cond.nonzero(parameter) end

---@param parameter string
---@param value da.Value
---@return da.Condition
function da.raw.cond.eq(parameter, value) end

---@param parameter string
---@param value da.Value
---@return da.Condition
function da.raw.cond.ne(parameter, value) end

---@param parameter string
---@param value da.Value
---@return da.Condition
function da.raw.cond.gt(parameter, value) end

---@param parameter string
---@param value da.Value
---@return da.Condition
function da.raw.cond.lt(parameter, value) end

return da
