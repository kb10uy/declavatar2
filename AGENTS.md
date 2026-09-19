# AGENTS.md

## Project Overview

Declavatar2 is a redesign of [declavatar v1](https://github.com/kb10uy/declavatar), a tool for declaratively defining and compiling/translating VRChat avatar data such as Expression Parameters, FX Controller, and Menu.
The overall architecture remains Rust core + C FFI + any language client (mainly C#/Unity).

## Packages

- **declavatar2** - Main library crate
- **da2** - C FFI library (da2.dll, libda2.so, libda2.dylib) crate

### Other Packages to be developed

- **declavatar2-vrchat** - VRChat/Unity binding for declavatar2 via da2. In declavatar v1, its equivalent is [modular-declavatar](https://github.com/kb10uy/modular-declavatar).

## Architecture

### Pipeline Overview

```
Lua script
    -> [mlua interpreter] -> Declaration
    -> [Transformer] -> Avatar (compiled, valid avatar data)
    -> [MessagePack serialization] -> consumed by clients
```

- **Keep the two-layer model (Declaration / Avatar).**
    - **Declaration** is a near 1:1 representation of what is written in the script. Each object is independent and does not enforce cross-object consistency.
    - **Avatar** is the final client-facing data model. Avatar data must be globally consistent as a whole.
- Dependency resolution and canonicalization happen while **transform process**, which takes Declaration and *compiles* into Avatar.
- The transform uses two passes.
    1. 1st pass for declaration collection.
    2. 2nd pass for reference resolution and type checks.

### Lua API Design

#### General

- Interpreter: Lua 5.4 via mlua.
- Scripts `require "declavatar"` (conventionally bound to `da`) and end with `return da.avatar(...)`, returning exactly one avatar (like Lua module convention).
- `da.avatar(blocks)` takes the blocks alone. A declaration carries no name of its own, because the client already knows which asset it is building.
- Builders are Rust-side mlua functions that return immutable userdata nodes. Invalid construction fails at the call site with the Lua traceback. No getters are exposed until a need arises.
- Every builder records the caller's chunk and line into `Unresolved.at` so transform errors can point at the script.
- Argument order: the trailing argument is always the child list; an optional options table precedes it (`da.group_layer(name, opts, children)`, `da.option(name, targets)`). `da.raw.field(position, motion)` and `da.raw.weighted(parameter, motion)` are the exceptions, their trailing argument is a single motion. `da.switch_layer` takes one or two trailing child lists and is the only builder whose options table is required, so its form is decided by argument count alone.
- Lists passed to builders are plain Lua tables.
    - `false` entries are skipped, so `cond and da.bool("X")` expresses a conditional element.
    - Nested lists are an error. Unlike declavatar v1, declavatar2 does not auto-flatten.
    - `da.flatten(...)` accepts nodes or lists of nodes, expands one level and drops `false`. Appending with `table.insert` is preferred where it reads naturally.
    - `da.map(list, fn)` is provided.
- Child lists may mix node kinds where noted; the builder sorts them by kind (`da.option` accepts animated targets and `da.drive_*`, `da.raw.layer` accepts states and transitions).
- Options tables are read by taking the known keys; any key left over is an error, so a typo fails at the call site.
- When two entries of one child list animate the same target, the later one silently wins. `Animation` is a `ValueSet`, and this is its ordinary overwrite behavior.
- `da.symbol("NAME")` returns whether the client supplied that symbol; use ordinary Lua control flow for conditional compilation.
- Repetition is expressed with ordinary Lua functions. Extension helpers such as `tracking_layer` are plain Lua modules (`require "declavatar.ext"`), not host features.

#### Values

- Vectors are written with `da.vec2(x, y)`, `da.vec3(x, y, z)` and `da.vec4(x, y, z, w)`. A plain table of two to four numbers is accepted as well.
- `da.color(r, g, b, a)` and `da.quat(x, y, z, w)` are required where those types are wanted; a bare table never becomes a color or a quaternion.
- `da.object(path):rotation(v)` takes Euler angles from a vector and a quaternion from `da.quat`.
- An integer option such as `da.int { default = ... }` takes a Lua integer only, and `1.5` is an error. A float option takes an integer too and converts it. Blend shape values are always floats.

#### Runtime and Modules

- The state is created without `io`, `os` and `debug`, and `package.searchers` is replaced so that no script can reach the host filesystem on its own.
- The replacement searchers run in order: preloaded modules (`declavatar`, `declavatar.ext`), the host loader supplied by the client, then the library directories given to the evaluator (`?.lua` and `?/init.lua`). Directory lookup reads files on the Rust side.
- `declavatar.ext` ships as Lua source embedded into the binary.
- Lua lives under `declavatar2/lua`, apart from the Rust sources. `runtime/` holds the Lua that is actually run and embedded, and `types/` holds lua-language-server definition files for `declavatar` and `declavatar.ext` so that an editor can complete a script. The definitions carry annotations only; a test compares them against the builders the runtime registers.
- No memory or instruction limit is imposed. A script that loops forever hangs the caller, at their own risk.

#### Parameters

- `da.bool(name, opts)`, `da.int(name, opts)`, `da.float(name, opts)` with `default`, `scope`, `save`, `width`.
- Parameter references are plain strings everywhere (`driven_by = "Emote"`).
- VRChat provided parameters are declared in bulk with `da.provided("VRChat")` inside `parameters`. Referencing one without the declaration is an error, and so is a user parameter whose name collides with a provided one.
- The group name of `da.provided` matches exactly, case included. An unknown name fails with the accepted names listed.

#### Animated Targets

- Targets come from bound objects instead of a layer-level default mesh:
    - `da.renderer(path[, type])` with `:shape(name[, value])`, `:material(slot, asset)`, `:property(name, value)`, `:reference(name, asset)`, `:enabled([bool])`.
    - `da.object(path)` with `:active([bool])`, `:position(v)`, `:rotation(v)`, `:scale(v)`.
    - `da.component(path, type)` with `:enabled([bool])`, `:property(name, value)`, `:reference(name, asset)`.
    - `da.animator_parameter(name, value)` for AAPs.
- `:property` on a renderer writes a material property (`_Color` and such). A serialized field of the renderer itself is written through `da.component(path, "UnityEngine.SkinnedMeshRenderer"):property(...)`, which is the same path any other component takes.
- `:reference` writes an object reference field, so its value is an asset rather than a number.
- The value type of `:property` follows the Lua value: boolean is `Bool`, integer is `Int`, other numbers are `Float`, a vector is `Vector2`/`Vector3`/`Vector4`, `da.color` is `Color` and `da.quat` is `Quaternion`.
- An omitted value means "full" (`1.0` / `true`); the consuming layer decides what "off" means.
- A string given to `:material` is `AssetLocator::Named` with the type `UnityEngine.Material`. `da.asset.guid(...)`, `da.asset.path(...)`, `da.asset.named(type, name)` give explicit locators, and `da.asset.named` always takes the type. There is no assets block; `Externals` collects every reference.
- Tracking control (`da.tracking(mode, targets)`) and parameter drives compile to state behaviors, never to animated values.
    - `mode` is `"tracking"` or `"animation"`, and a target is `"head"`, `"left_hand"`, `"right_hand"`, `"hip"`, `"left_foot"`, `"right_foot"`, `"left_fingers"`, `"right_fingers"`, `"eyes"` or `"mouth"`.

#### State Behaviors

- Parameter drives and tracking control stay typed, because the transform has to understand them: drives resolve layer references into concrete parameter values.
- Any other state behavior is carried verbatim by `GenericStateBehavior`, which holds a type name and a tree of `GenericValue` (bool, integer, float, string, list, map). The transform passes it through untouched and the client feeds it to the actual component.
- A generic payload holds plain data only. Parameter names, object paths and assets are not resolved or interned inside it, so it never takes part in reference checking.
- The Lua builder for it is not exposed yet; only the data model exists.

#### Layers

- `da.group_layer(name, { driven_by, symmetric }, { da.default { ... }, da.option(name, targets), ... })`.
    - Option indices are assigned automatically during transform; declarations cannot specify them. References such as `da.drive_group(layer, option)` resolve the generated value from the layer and option. Use raw layers when parameter values are externally constrained.
    - Completion is always mutual-zeroed: the default absorbs the zeroed union of every option's keys (`ValueSet::union_fill_as_zero`), then each option inherits the default entries it lacks (`ValueSet::union_from_defaults`). There is no copy mode.
    - A non-zeroable entry (object reference) used by an option but missing from the default is an error.
    - `symmetric` chooses the state machine shape only; clip contents are the same either way.
        - `true` (default): every state is equal. Entry fans out to each option on `== index` and falls back to the default state, each option exits on `!= index`, the default state exits on each `== index`. Switching between options never passes through the default state, so its behaviors do not run on the way. Entry transitions carry no duration; a crossfade is set on the exit transitions and applies to whatever state Entry resolves to, so it is per source state, not per destination.
        - `false`: the v1 hub. The default state transitions to each option on `== index`, each option returns to the default state on `!= index`. Every switch passes through the default state for one frame and runs its behaviors; use this when that pass or per-transition durations are wanted.
    - Direct blend tree output is not generated from a group layer. Write `da.raw.state` with `da.raw.blend_tree({ type = "linear" })` for that.
- `da.switch_layer(name, { driven_by | gate }, enabled)` or `da.switch_layer(name, { driven_by | gate }, disabled, enabled)`.
    - The options table is required (pass `{}` when empty) so that three arguments always mean a toggle list and four always mean both sides. Table shapes are never inspected to tell the forms apart.
    - The three-argument form is a toggle list: on gets the given or full value, off gets the zeroed value. Explicit `false` or zero values in a toggle list are an error.
    - The four-argument form spells both sides out in `disabled, enabled` order, matching `false, true`. An empty `disabled` list writes nothing in the off state, unlike a toggle list which writes zeroed values.
- `da.puppet_layer(name, { driven_by }, { da.keyframe(t, targets), ... })`.
    - Compiles to one state holding a 1D linear blend tree, not a motion-time clip: each keyframe becomes a fixed clip placed at threshold `t`, so keyframes are joined with linear interpolation. `driven_by` must be a float, and `t` is any real value rather than normalized time, so a `-1..1` puppet axis is used as is.
    - A target missing from a keyframe is filled by linearly interpolating its neighbours, and held at the ends. Every generated clip writes the same key set.
    - Step or `Curve` interpolation is not exposed here; write `da.raw.clip({ time_by }, targets)` for that.
- `da.blend_layer(name, { da.puppet_layer(...), ... })` merges its children into one layer whose single state is a direct blend tree. Merging is explicit; layers written at the top level always stay separate.
    - Only layers that have no behaviors and are driven by a float can be merged, which is currently puppet layers only. Group and switch layers would need a float mirror of their parameter and are not accepted.
    - Each child becomes a direct field weighted by a float animator parameter fixed at `1.0`; the transform adds that parameter and it is not an expression parameter. The layer is Write Defaults on.
    - Children sum instead of overriding, so a target animated by two children of the same blend layer is an error. Overlap with other layers keeps the usual layer-order override.
    - The merged layer sits where the `da.blend_layer` is written in `fx_controller`. Drives such as `da.drive_puppet` keep referencing the child layer by name.
- `exports = { da.gate(name), da.guard(gate, parameter) }`.

#### Raw Layers (`da.raw.*`)

- `da.raw.layer(name, { default = state }, { states and/or transitions })`.
- `da.raw.state(name, { motion, behaviors }, { outgoing transitions })`.
- `da.raw.transition([from,] to[, opts], conditions)` with `duration`. `from` is implicit inside a state's child list and required in a layer's child list. Both forms compile to the same flat transition list.
    - Three arguments are ambiguous between `(from, to, conditions)` and `(to, opts, conditions)`. A table in the second position means the options table, a state reference means `to`. The contents of the table are never inspected.
- State references (`default`, `from`, `to`) accept a name string or a state object. Forward references must be strings.
- Motions: `da.raw.clip([opts,] targets)` with `speed`, `speed_by`, `time_by`; `da.raw.external(asset[, opts])`; `da.raw.blend_tree({ type, x[, y] }, { da.raw.field(position, motion), ... })` with `type` in `linear`, `simple_2d`, `freeform_2d`, `cartesian_2d`; `da.raw.blend_tree({ type = "direct" }, { da.raw.weighted(parameter, motion), ... })`. A direct tree accepts only weighted fields and the others only positioned fields.
- Conditions live under `da.raw.cond`: `zero`, `nonzero`, `eq`, `ne`, `gt`, `lt`. The comparison type comes from the parameter type in the 2nd pass; unsupported combinations such as `eq` on a float are errors.

#### Menu

- `da.submenu(name, items)`, `da.toggle(name, drive)`, `da.button(name, drive)`, `da.radial(name, axis)`, two-axis and four-axis puppets.
- An axis is a parameter name, a `da.drive_puppet(...)`, or `da.axis(target, { positive, negative })` when labels are needed.
- `da.two_axis(name, { horizontal, vertical })` and `da.four_axis(name, { up, down, left, right })` name their axes rather than ordering them, because four directions in a row read as a puzzle. Every axis is required and an unknown key is an error, as in any options table.
- Drives: `da.drive_group(layer, option)`, `da.drive_switch(layer[, bool])`, `da.drive_puppet(layer[, value])`, `da.drive_bool(parameter, value)`, `da.drive_int(parameter, value)`, `da.drive_float(parameter, value)`.

```lua
local da = require "declavatar"

local Face = da.renderer("Face")
local hat = da.symbol("ENABLE_HAT")

return da.avatar({
    parameters = {
        da.provided("VRChat"),
        da.int("Emote", { default = 42 }),
        hat and da.bool("Hat", { scope = "local" }),
    },
    fx_controller = {
        da.group_layer("Expressions", { driven_by = "Emote" }, {
            da.default { Face:shape("eyelid_L", 0.3) },
            da.option("smile", { Face:shape("smile"), Face:shape("eye_joy", 0.5) }),
        }),
        hat and da.switch_layer("Hat", { driven_by = "Hat" }, { da.object("Hat"):active() }),
    },
    menu = {
        hat and da.toggle("Hat", da.drive_switch("Hat")),
    },
})
```

### Interop Format

- Use **MessagePack** instead of JSON used in v1.
- Rust side: `rmp-serde` (direct output from serde `Serialize`).
- C# side: `MessagePack-CSharp` (with Source Generator support).
- Simplify the C FFI API to a single compile -> MessagePack blob flow.

### External References

- v1 took a `Map<String, Asset>` ScriptableObject as compiler input. v2 reverses that: compilation needs nothing but the script and the symbols, and the resulting avatar enumerates what it requires.
- `Externals` is that enumeration. Object paths, component types and assets each get an `ExternTable`, deduplicated, and the avatar body refers to them by index. Every entry carries `referenced_at`, so an unmet requirement is reported against the line that asked for it.
- The client resolves the tables in index order and builds one array per kind, then reads the avatar body straight through it. No second compilation pass is involved.
- Resolving an asset is the client's job, as is checking that what it found matches the `asset_type` of a `Named` locator.
- A dictionary ScriptableObject equivalent to v1's is still accepted, as one source of resolutions rather than a compiler input. For `AssetLocator::Named` the dictionary wins, and a project-wide search is used only when it hits exactly one asset.
- Losing compile-time name checking is the deliberate cost. Reporting a failure at the right script line keeps it manageable.

## Code Style

- Rust edition 2024
- Avoid comments by default (add comments only when explicitly requested by the user).
- Utilize rstest features.

## Build & Test

- Rust workspace layout
- Build and test with `cargo build` and `cargo test`
- lint/typecheck: `cargo clippy`, `cargo fmt --check`
