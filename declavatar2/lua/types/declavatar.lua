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

--- Entry of the `controllers` block: layers bound for one playable layer.
---@class da.Controller

--- Layer inside a controller.
---@class da.Layer

--- Default state of a group layer.
---@class da.GroupDefault

--- One option of a group layer.
---@class da.GroupOption

--- One keyframe of a puppet layer.
---@class da.Keyframe

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

--- State machine nested in a raw layer.
---@class da.Machine

--- Entry of a state machine, the value `da.raw.entry`.
---@class da.Entry

--- Exit of a state machine, the value `da.raw.exit`.
---@class da.Exit

--- Transition between two states of a raw layer.
---@class da.Transition

--- What a raw state plays.
---@class da.Motion

--- One keyframe of a keyed clip.
---@class da.ClipKeyframe

--- Bezier easing of a keyed clip segment.
---@class da.Bezier

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

--- Playable layer of the avatar descriptor a controller is bound for.
---@alias da.PlayableLayer "base"|"additive"|"gesture"|"action"|"fx"|"sitting"|"tpose"|"ikpose"

--- Whether a controller is appended to the playable layer or replaces it as a whole.
---@alias da.MergeMode "append"|"replace"

--- What the object paths of a controller start at: the avatar root, or a root the client supplies.
---@alias da.PathMode "absolute"|"relative"

--- Whether a layer replaces what the layers before it animate, or adds to it.
---@alias da.LayerBlending "override"|"additive"

--- What tracking control does to the parts it names.
---@alias da.TrackingMode "tracking"|"animation"

--- Part of the body that tracking control acts on.
---@alias da.TrackingTarget "head"|"left_hand"|"right_hand"|"hip"|"left_foot"|"right_foot"|"left_fingers"|"right_fingers"|"eyes"|"mouth"

--- Playable layer whose weight a state can blend.
---@alias da.BlendablePlayable "additive"|"gesture"|"action"|"fx"

--- Whether `da.pose_space` moves the viewpoint to the head or back.
---@alias da.PoseSpace "enter"|"exit"

--- Which clip `da.play_audio` plays next.
---@alias da.PlaybackOrder "random"|"unique_random"|"roundabout"|"parameter"

--- When a setting of `da.play_audio` is written to the AudioSource.
---@alias da.AudioApply "always"|"if_stopped"|"never"

--- How a blend tree blends its fields. A `direct` tree weights each field by its own parameter.
---@alias da.BlendTreeType "linear"|"simple_2d"|"freeform_2d"|"cartesian_2d"|"direct"

--- How a keyed clip segment moves from one keyframe to the next, besides `da.raw.bezier`.
---@alias da.InterpolationName "constant"|"linear"

--- How a keyed clip segment moves from one keyframe to the next.
---@alias da.Interpolation da.InterpolationName|da.Bezier

--- Value written for an animated property. A plain table of two to four numbers is a vector.
---@alias da.Value boolean|number|da.Vector|da.Color|da.Quaternion|number[]

--- Value of a generic state behavior field: plain data only. A table with string keys is a map,
--- a sequence is a list, and an empty table is an empty list. `false` is kept as a value here.
---@alias da.GenericValue boolean|integer|number|string|da.GenericValue[]|table<string, da.GenericValue>

--- Range written as two numbers, `{ min, max }`.
---@alias da.Range da.Vector|[number, number]

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

--- State or nested state machine, written as its name or as the state or machine itself.
---@alias da.NodeValue string|da.State|da.Machine

--- Where a transition leaves: a state, the exit of a nested machine, or the entry of the machine holding it.
---@alias da.SourceValue da.NodeValue|da.Entry

--- Where a transition leads: a state, the entry of a nested machine, or the exit of the machine holding it.
---@alias da.TargetValue da.NodeValue|da.Exit

--- Place of a blend tree field: one number on a single axis, or two on a pair of them.
---@alias da.Position number|da.Vector|[number, number]

--------------------------------------------------------------------------------
-- Child lists
--------------------------------------------------------------------------------
-- `false` is dropped from every child list, so `cond and da.bool("X")` reads as a
-- conditional element. Nested lists are an error; use `da.flatten` instead.

---@alias da.ParameterList (da.Parameter|false)[]
---@alias da.ControllerList (da.Controller|false)[]
---@alias da.LayerList (da.Layer|false)[]
---@alias da.MenuItemList (da.MenuItem|false)[]
---@alias da.ContentList (da.Target|da.Drive|da.Behavior|false)[]
---@alias da.TargetList (da.Target|false)[]
---@alias da.BehaviorList (da.Drive|da.Behavior|false)[]
---@alias da.GroupChildList (da.GroupDefault|da.GroupOption|false)[]
---@alias da.KeyframeList (da.Keyframe|false)[]
---@alias da.ClipKeyframeList (da.ClipKeyframe|false)[]
---@alias da.RawChildList (da.State|da.Machine|da.Transition|false)[]
---@alias da.TransitionList (da.Transition|false)[]
---@alias da.ConditionList (da.Condition|false)[]
---@alias da.FieldList (da.Field|false)[]
---@alias da.WeightedFieldList (da.WeightedField|false)[]
---@alias da.TrackingTargetList (da.TrackingTarget|false)[]
---@alias da.AudioClipList (da.AssetValue|false)[]

--------------------------------------------------------------------------------
-- Options tables
--------------------------------------------------------------------------------
-- A key an options table does not know is an error, so a typo fails at the call site.

--- Blocks of an avatar.
---@class da.AvatarBlocks
---@field parameters? da.ParameterList
---@field controllers? da.ControllerList
---@field menu? da.MenuItemList

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

--- How a controller is applied. Every option has a default, so the table may be left out.
---@class da.ControllerOptions
---@field mode? da.MergeMode Defaults to `"append"`.
---@field priority? integer Order among controllers bound for the same playable layer. Defaults to `0`.
---@field path_mode? da.PathMode Defaults to `"absolute"`.
---@field mask? da.Asset Avatar mask of every layer that does not have its own.

--- Options every layer takes.
---@class da.LayerOptions
---@field weight? number Weight the layer starts with, between 0 and 1. Defaults to 1.
---@field blending? da.LayerBlending Defaults to `"override"`.
---@field mask? da.Asset Avatar mask of this layer, used in place of the mask of its controller.

---@class da.GroupLayerOptions: da.LayerOptions
---@field driven_by? string
---@field symmetric? boolean Defaults to true, where switching between options never passes through the default state.

---@class da.SwitchLayerOptions: da.LayerOptions
---@field driven_by? string

--- A puppet layer merged into a blend layer is not a layer of its own, so it takes none of `da.LayerOptions`.
---@class da.PuppetLayerOptions: da.LayerOptions
---@field driven_by? string Must be a float.

---@class da.MachineOptions
---@field default? da.StateValue Must be a state the machine holds directly. Defaults to its first state.

---@class da.RawLayerOptions: da.LayerOptions, da.MachineOptions

---@class da.RawStateOptions
---@field motion? da.Motion
---@field behaviors? da.BehaviorList

---@class da.TransitionOptions
---@field duration? number

---@class da.ClipOptions
---@field speed? number
---@field speed_by? string
---@field time_by? string

--- Playback of a keyed clip, together with the clip settings Unity keeps on the clip itself.
---@class da.KeyedClipOptions: da.ClipOptions
---@field length? number Seconds that normalized time 0 to 1 spans. Positive; defaults to 1.
---@field loop_time? boolean Defaults to false.
---@field loop_blend? boolean Defaults to false.
---@field cycle_offset? number Defaults to 0.

---@class da.ClipKeyframeOptions
---@field interpolation? da.Interpolation How each target written here arrives from its previous keyframe. Defaults to `"linear"` for values that can be interpolated and `"constant"` for the others.

--- Ranges of a ranged copy, written together.
---@class da.CopyOptions
---@field from_range? da.Range
---@field to_range? da.Range

---@class da.WeightOptions
---@field goal_weight? number Between 0 and 1. Defaults to 1.
---@field blend_duration? number Seconds the weight takes to reach the goal. Defaults to 0.

---@class da.PoseSpaceOptions
---@field delay? number Defaults to 0.
---@field fixed_delay? boolean Whether `delay` is in seconds rather than a fraction of the state. Defaults to true.

---@class da.PlayAudioOptions
---@field order? da.PlaybackOrder Defaults to `"parameter"` when `parameter` is written and `"random"` otherwise.
---@field parameter? string Int parameter holding the index of the clip to play, read by order `"parameter"`.
---@field volume? da.Range Random range between 0 and 1. Defaults to `{ 1, 1 }`.
---@field pitch? da.Range Random range between -3 and 3. Defaults to `{ 1, 1 }`.
---@field loop? boolean Defaults to false.
---@field delay? number Seconds between entering the state and playing, up to 60. Defaults to 0.
---@field play_on_enter? boolean Defaults to true.
---@field stop_on_enter? boolean Defaults to true.
---@field play_on_exit? boolean Defaults to false.
---@field stop_on_exit? boolean Defaults to false.
---@field clips_apply? da.AudioApply Defaults to `"if_stopped"`.
---@field volume_apply? da.AudioApply Defaults to `"if_stopped"`.
---@field pitch_apply? da.AudioApply Defaults to `"if_stopped"`.
---@field loop_apply? da.AudioApply Defaults to `"if_stopped"`.

--- A parametric tree blends along `x`, and a two dimensional one along `y` as well.
--- A `direct` tree has neither.
---@class da.BlendTreeOptions
---@field type da.BlendTreeType
---@field x? string
---@field y? string

--- Labels of the two ends of an axis. Only a two-axis puppet shows both; a four-axis
--- direction shows `positive` alone, and a radial puppet shows neither.
---@class da.AxisOptions
---@field positive? string
---@field negative? string

---@class da.TwoAxisOptions
---@field horizontal da.AxisValue
---@field vertical da.AxisValue

--- Each direction shows the `positive` label of its axis; `negative` is an error.
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

--- Material property such as `_Color`. Object references such as textures cannot be
--- animated on a material property; swap the whole material with `:material` instead.
---@param name string
---@param value da.Value
---@return da.Target
function Renderer:property(name, value) end

--- Serialized field of the renderer itself, such as `m_UpdateWhenOffscreen`.
--- Its type follows the value written for it, and `da.asset.*` writes an object reference.
---@param name string
---@param value da.Value|da.Asset
---@return da.Target
function Renderer:serialized(name, value) end

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

--- Adds to an int or float parameter. Like the other drives below, this only runs in a state,
--- so it is a behavior and cannot trigger a menu item.
---@param parameter string
---@param value number
---@return da.Behavior
function da.drive_add(parameter, value) end

--- Sets an int parameter to a random value between `min` and `max`, both included.
---@param parameter string
---@param min integer
---@param max integer
---@return da.Behavior
function da.drive_random_int(parameter, min, max) end

--- Sets a bool parameter to true with the given chance.
---@param parameter string
---@param chance? number Between 0 and 1. Defaults to 0.5.
---@return da.Behavior
function da.drive_random_bool(parameter, chance) end

--- Sets a float parameter to a random value between `min` and `max`.
---@param parameter string
---@param min number
---@param max number
---@return da.Behavior
function da.drive_random_float(parameter, min, max) end

--- Copies one parameter into another. With both ranges written, `from_range` is mapped onto `to_range`.
---@param from string
---@param to string
---@param options? da.CopyOptions
---@return da.Behavior
function da.drive_copy(from, to, options) end

--- Hands the named parts over to animation, or back to tracking.
---@param mode da.TrackingMode
---@param targets da.TrackingTargetList
---@return da.Behavior
function da.tracking(mode, targets) end

--- State behavior of any other type, whose fields are written as plain data and applied as they are.
--- Parameter names, object paths and assets inside the fields are not checked.
---@param type_name string Fully qualified type name, looked up by the client like a component type.
---@param fields? table<string, da.GenericValue>
---@return da.Behavior
function da.behavior(type_name, fields) end

--- Blends the weight of a layer declared in this avatar.
---
--- The layer belongs to the controller holding the state, which is action, fx, gesture or
--- additive. A layer of another controller cannot be named, even one of the same playable layer,
--- because the client may finalize each controller on its own, where such an index is unknown.
--- A child of `da.blend_layer` is merged into it and cannot be named on its own.
---@param layer string
---@param options? da.WeightOptions
---@return da.Behavior
function da.layer_control(layer, options) end

--- Blends the weight of a whole playable layer.
---@param playable da.BlendablePlayable
---@param options? da.WeightOptions
---@return da.Behavior
function da.playable_control(playable, options) end

--- Turns locomotion off, or back on.
---@param enabled boolean
---@return da.Behavior
function da.locomotion(enabled) end

--- Moves the viewpoint to the head, or back to where it was.
---@param mode da.PoseSpace
---@param options? da.PoseSpaceOptions
---@return da.Behavior
function da.pose_space(mode, options) end

--- Plays clips on an AudioSource when the state is entered or left.
---
--- `source` is the path of the object holding the AudioSource, read like any other object path of
--- the controller; an empty string is the root that those paths start at. A bare clip name is an
--- `UnityEngine.AudioClip`.
---@param source string
---@param options da.PlayAudioOptions
---@param clips da.AudioClipList
---@return da.Behavior
---@overload fun(source: string, clips: da.AudioClipList): da.Behavior
function da.play_audio(source, options, clips) end

--------------------------------------------------------------------------------
-- Layers
--------------------------------------------------------------------------------

--- Layers bound for one playable layer, with how the client applies them.
---
--- The same playable layer may be written more than once; each entry is applied on its own,
--- ordered by `priority`. Layer names are unique across every controller of the avatar.
--- With `path_mode = "relative"`, every object path in the controller starts at a root the
--- client supplies instead of the avatar root.
---@param playable da.PlayableLayer
---@param options da.ControllerOptions
---@param layers da.LayerList
---@return da.Controller
---@overload fun(playable: da.PlayableLayer, layers: da.LayerList): da.Controller
function da.controller(playable, options, layers) end

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
--- The layer options belong to the blend layer, not to its children.
---@param name string
---@param options da.LayerOptions
---@param children da.LayerList
---@return da.Layer
---@overload fun(name: string, children: da.LayerList): da.Layer
function da.blend_layer(name, options, children) end

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

--- Radial puppet. VRChat shows no labels on it, so `da.axis` given here takes no labels.
---@param name string
---@param axis da.AxisValue
---@return da.MenuItem
function da.radial(name, axis) end

--- Two axis puppet. Both axes are named rather than ordered.
---@param name string
---@param axes da.TwoAxisOptions
---@return da.MenuItem
function da.two_axis(name, axes) end

--- Four axis puppet. Every direction is named rather than ordered, and shows one label.
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
---
--- A name written in a machine refers to a state or a state machine that machine holds
--- directly, and states and machines share those names. A transition never crosses the
--- boundary of a machine; it goes through the entry and the exit instead.
---@param name string
---@param options da.RawLayerOptions
---@param children da.RawChildList
---@return da.Layer
---@overload fun(name: string, children: da.RawChildList): da.Layer
function da.raw.layer(name, options, children) end

--- State machine nested in a raw layer or in another machine.
---
--- A transition leading to the machine enters it through its entry: the transitions leaving
--- `da.raw.entry` inside it choose the state, and the default state is taken when none of them
--- holds. A transition leading to `da.raw.exit` inside it leaves the machine, and the transitions
--- leaving the machine in its parent choose where to go on.
---@param name string
---@param options da.MachineOptions
---@param children da.RawChildList
---@return da.Machine
---@overload fun(name: string, children: da.RawChildList): da.Machine
function da.raw.machine(name, options, children) end

--- Entry of the machine holding a transition, written as the place the transition leaves.
---@type da.Entry
da.raw.entry = nil

--- Exit of the machine holding a transition, written as the place the transition leads.
---@type da.Exit
da.raw.exit = nil

--- One state of a raw layer. A transition written in `outgoing` leaves this state.
---@param name string
---@param options? da.RawStateOptions
---@param outgoing? da.TransitionList
---@return da.State
function da.raw.state(name, options, outgoing) end

--- Transition between two nodes of one state machine.
---
--- Inside a state the source is implied, so `from` is left out. A table in the second
--- place is the options table, which is how the three argument forms are told apart.
---
--- Leaving a state with an empty condition list, the transition is taken once the motion of
--- the state has played to the end (exit time 1). Leaving `da.raw.entry` or a nested machine,
--- the transition is chosen at the moment that place is passed, is taken at once with an empty
--- condition list, and has no `duration`.
---@param from da.SourceValue
---@param to da.TargetValue
---@param options da.TransitionOptions
---@param conditions da.ConditionList
---@return da.Transition
---@overload fun(to: da.TargetValue, conditions: da.ConditionList): da.Transition
---@overload fun(from: da.SourceValue, to: da.TargetValue, conditions: da.ConditionList): da.Transition
---@overload fun(to: da.TargetValue, options: da.TransitionOptions, conditions: da.ConditionList): da.Transition
function da.raw.transition(from, to, options, conditions) end

--- Clip generated from the written targets.
---@param options da.ClipOptions
---@param targets da.TargetList
---@return da.Motion
---@overload fun(targets: da.TargetList): da.Motion
function da.raw.clip(options, targets) end

--- Clip whose targets follow curves through keyframes.
---
--- Keyframe times are normalized: 0 is the start of the clip and 1 is its end, and `length`
--- says how many seconds that spans. Each target gets its own curve through the keyframes it
--- is written in, which are ordered by time; a target written in no keyframe is not animated.
---@param options da.KeyedClipOptions
---@param keyframes da.ClipKeyframeList
---@return da.Motion
---@overload fun(keyframes: da.ClipKeyframeList): da.Motion
function da.raw.keyed_clip(options, keyframes) end

--- One keyframe of a keyed clip, at a normalized time between 0 and 1.
---@param time number
---@param options da.ClipKeyframeOptions
---@param targets da.TargetList
---@return da.ClipKeyframe
---@overload fun(time: number, targets: da.TargetList): da.ClipKeyframe
function da.raw.keyframe(time, options, targets) end

--- Easing in the CSS `cubic-bezier` convention over one segment. `x1` and `x2` are between 0 and 1.
---@param x1 number
---@param y1 number
---@param x2 number
---@param y2 number
---@return da.Bezier
function da.raw.bezier(x1, y1, x2, y2) end

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
