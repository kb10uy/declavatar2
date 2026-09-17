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
- Builders are Rust-side mlua functions that return immutable userdata nodes. Invalid construction fails at the call site with the Lua traceback. No getters are exposed until a need arises.
- Every builder records the caller's chunk and line into `Unresolved.at` so transform errors can point at the script.
- Argument order: the trailing argument is always the child list; an optional options table precedes it (`da.group_layer(name, opts, children)`, `da.option(name, targets)`). `da.raw.field(position, motion)` is the one exception, its trailing argument is a single motion.
- Lists passed to builders are plain Lua tables.
    - `false` entries are skipped, so `cond and da.bool("X")` expresses a conditional element.
    - Nested lists are an error. Unlike declavatar v1, declavatar2 does not auto-flatten.
    - `da.flatten(...)` accepts nodes or lists of nodes, expands one level and drops `false`. Appending with `table.insert` is preferred where it reads naturally.
    - `da.map(list, fn)` is provided.
- Child lists may mix node kinds where noted; the builder sorts them by kind (`da.option` accepts animated targets and `da.drive_*`, `da.raw.layer` accepts states and transitions).
- `da.symbol("NAME")` returns whether the client supplied that symbol; use ordinary Lua control flow for conditional compilation.
- Repetition is expressed with ordinary Lua functions. Extension helpers such as `tracking_layer` are plain Lua modules (`require "declavatar.ext"`), not host features.

#### Parameters

- `da.bool(name, opts)`, `da.int(name, opts)`, `da.float(name, opts)` with `default`, `scope`, `save`, `width`.
- Parameter references are plain strings everywhere (`driven_by = "Emote"`).
- VRChat provided parameters are declared in bulk with `da.provided("VRChat")` inside `parameters`. Referencing one without the declaration is an error, and so is a user parameter whose name collides with a provided one.

#### Animated Targets

- Targets come from bound objects instead of a layer-level default mesh:
    - `da.renderer(path[, type])` with `:shape(name[, value])`, `:material(slot, asset)`, `:property(name, value)`, `:enabled([bool])`.
    - `da.object(path)` with `:active([bool])`, `:position(v)`, `:rotation(v)`, `:scale(v)`.
    - `da.component(path, type)` with `:enabled([bool])`, `:property(name, value)`.
    - `da.animator_parameter(name, value)` for AAPs.
- An omitted value means "full" (`1.0` / `true`); the consuming layer decides what "off" means.
- A string given to `:material` is `AssetLocator::Named`. `da.asset.guid(...)`, `da.asset.path(...)`, `da.asset.named(type, name)` give explicit locators. There is no assets block; `Externals` collects every reference.
- Tracking control (`da.tracking(mode, targets)`) and parameter drives compile to state behaviors, never to animated values.

#### Layers

- `da.group_layer(name, { driven_by }, { da.default { ... }, da.option(name[, { index }], targets), ... })`.
    - Completion is always mutual-zeroed: the default absorbs the zeroed union of every option's keys (`ValueSet::union_fill_as_zero`), then each option inherits the default entries it lacks (`ValueSet::union_from_defaults`). There is no copy mode.
    - A non-zeroable entry (object reference) used by an option but missing from the default is an error.
- `da.switch_layer(name, { driven_by | gate }, children)`.
    - A sequence is a toggle list: on gets the given or full value, off gets the zeroed value. Explicit `false` or zero values in a toggle list are an error.
    - `{ off = { ... }, on = { ... } }` spells both sides out.
- `da.puppet_layer(name, { driven_by }, { da.keyframe(t, targets), ... })`. Keyframes are joined with linear interpolation; `Curve` interpolation is not exposed for now.
- `exports = { da.gate(name), da.guard(gate, parameter) }`.

#### Raw Layers (`da.raw.*`)

- `da.raw.layer(name, { default = state }, { states and/or transitions })`.
- `da.raw.state(name, { motion, behaviors }, { outgoing transitions })`.
- `da.raw.transition([from,] to[, opts], conditions)` with `duration`. `from` is implicit inside a state's child list and required in a layer's child list. Both forms compile to the same flat transition list.
- State references (`default`, `from`, `to`) accept a name string or a state object. Forward references must be strings.
- Motions: `da.raw.clip([opts,] targets)` with `speed`, `speed_by`, `time_by`; `da.raw.external(asset[, opts])`; `da.raw.blend_tree({ type, x[, y] }, { da.raw.field(position, motion), ... })` with `type` in `linear`, `simple_2d`, `freeform_2d`, `cartesian_2d`.
- Conditions live under `da.raw.cond`: `zero`, `nonzero`, `eq`, `ne`, `gt`, `lt`. The comparison type comes from the parameter type in the 2nd pass; unsupported combinations such as `eq` on a float are errors.

#### Menu

- `da.submenu(name, items)`, `da.toggle(name, drive)`, `da.button(name, drive)`, `da.radial(name, axis)`, two-axis and four-axis puppets.
- An axis is a parameter name, a `da.drive_puppet(...)`, or `da.axis(target, { positive, negative })` when labels are needed.
- Drives: `da.drive_group(layer, option)`, `da.drive_switch(layer[, bool])`, `da.drive_puppet(layer[, value])`, `da.drive_bool(parameter, value)`, `da.drive_int(parameter, value)`, `da.drive_float(parameter, value)`.

```lua
local da = require "declavatar"

local Face = da.renderer("Face")
local hat = da.symbol("ENABLE_HAT")

return da.avatar("name", {
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

## Code Style

- Rust edition 2024
- Avoid comments by default (add comments only when explicitly requested by the user).
- Utilize rstest features.

## Build & Test

- Rust workspace layout
- Build and test with `cargo build` and `cargo test`
- lint/typecheck: `cargo clippy`, `cargo fmt --check`
