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
    -> [interop encoding] -> blob consumed by clients
```

- **Keep the two-layer model (Declaration / Avatar).**
    - **Declaration** is a near 1:1 representation of what is written in the script. Each object is independent and does not enforce cross-object consistency.
    - **Avatar** is the final client-facing data model. Avatar data must be globally consistent as a whole.
- Dependency resolution and canonicalization happen while **transform process**, which takes Declaration and *compiles* into Avatar.
- The transform uses two passes.
    1. 1st pass for declaration collection.
    2. 2nd pass for reference resolution and type checks.
- Shared Unity data structures are generic over `Phase`. `Declared` uses `Unresolved` names and asset locators; `Compiled` uses typed `Resolved` parameter names and `Extern` indices for client-side references. Source locations belong to references and declaration nodes, not to reference identity.

### Lua API Design

#### General

- Interpreter: Lua 5.4 via mlua.
- Scripts `require "declavatar"` (conventionally bound to `da`) and end with `return da.avatar(...)`, returning exactly one avatar (like Lua module convention).
- `da.avatar(blocks)` takes the blocks alone. A declaration carries no name of its own, because the client already knows which asset it is building.
- The avatar blocks are `parameters`, `controllers` and `menu`, all optional. There is no `exports` block or gate/guard feature. Use explicitly declared parameters with `scope = "internal"` for animator-only state, and raw layers for custom conditions and transitions.
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
- Repetition is expressed with ordinary Lua functions. Extension helpers belong in plain Lua modules (`require "declavatar.ext"`), not host features. The embedded extension currently provides `range(from, to[, step])`; `tracking_layer` is not implemented.

#### Values

- Vectors are written with `da.vec2(x, y)`, `da.vec3(x, y, z)` and `da.vec4(x, y, z, w)`. A plain table of two to four numbers is accepted as well.
- `da.color(r, g, b, a)` and `da.quat(x, y, z, w)` are required where those types are wanted; a bare table never becomes a color or a quaternion.
- `da.object(path):rotation(v)` takes Euler angles from a vector and a quaternion from `da.quat`.
- An integer option such as `da.int("Emote", { default = ... })` takes a Lua integer only, and `1.5` is an error. A float option takes an integer too and converts it. Blend shape values are always floats.

#### Runtime and Modules

- The state is created without `io`, `os` and `debug`, and `package.searchers` is replaced so that no script can reach the host filesystem on its own.
- The replacement searchers consult preloaded modules (`declavatar`, `declavatar.ext`) first, then the loaders in the order added to `EvaluateOptions`. Both `loader(...)` and `library_paths(...)` append loaders, so the client chooses their precedence. Directory lookup tries `?.lua` and `?/init.lua` and reads files on the Rust side.
- `declavatar.ext` ships as Lua source embedded into the binary.
- Lua lives under `declavatar2/lua`, apart from the Rust sources. `runtime/` holds the Lua that is actually run and embedded, and `types/` holds lua-language-server definition files for `declavatar` and `declavatar.ext` so that an editor can complete a script. The definitions carry annotations only; a test compares them against the builders the runtime registers.
- No memory or instruction limit is imposed. A script that loops forever hangs the caller, at their own risk.

#### Parameters

- `da.bool(name, opts)`, `da.int(name, opts)`, `da.float(name, opts)` with `default`, `scope`, `save`; `width` is accepted only by int and float parameters.
- Parameter references are plain strings everywhere (`driven_by = "Emote"`).
- VRChat provided parameters are declared in bulk with `da.provided("VRChat")` inside `parameters`. Referencing one without the declaration is an error, and so is a user parameter whose name collides with a provided one.
- The group name of `da.provided` matches exactly, case included. An unknown name fails with the accepted names listed.

#### Animated Targets

- Targets come from bound objects instead of a layer-level default mesh:
    - `da.renderer(path[, type])` with `:shape(name[, value])`, `:material(slot, asset)`, `:property(name, value)`, `:reference(name, asset)`, `:enabled([bool])`.
    - `da.object(path)` with `:active([bool])`, `:position(v)`, `:rotation(v)`, `:scale(v)`.
    - `da.component(path, type)` with `:enabled([bool])`, `:property(name, value)`, `:reference(name, asset)`.
    - `da.animator_parameter(name, value)` for AAPs; the referenced animator parameter must exist and have float type.
- `:property` on a renderer writes a material property (`_Color` and such). A serialized field of the renderer itself is written through `da.component(path, "UnityEngine.SkinnedMeshRenderer"):property(...)`, which is the same path any other component takes.
- `:reference` writes an object reference field and requires an explicit `da.asset.guid(...)`, `da.asset.path(...)` or `da.asset.named(type, name)` locator. A bare string is rejected because the asset type cannot be inferred.
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

#### Controllers

- `controllers = { da.controller(playable, { mode, priority, path_mode, mask }, { layers... }), ... }`. There is no `fx_controller` block; every layer lives inside a `da.controller`.
    - `playable` is `"base"`, `"additive"`, `"gesture"`, `"action"`, `"fx"`, `"sitting"`, `"tpose"` or `"ikpose"`. The same playable layer may be written more than once; each entry is applied on its own, so a low-priority base and a high-priority override can sit in one script.
    - The options map onto what a Modular Avatar Merge Animator takes: `mode` is `"append"` (default) or `"replace"`, `priority` is an integer defaulting to `0`, `mask` is an explicit `da.asset.*` locator. Delete Attached Animator, Match Avatar Write Defaults and Relative Path Root are component-side settings and are not written in a script.
    - `path_mode` is `"absolute"` (default) or `"relative"`. Object paths in a relative controller start at a root the client supplies instead of the avatar root. A bound object such as `da.object("Hat")` is only a path, so the same object used from controllers of both modes names two different objects; the transform does not check for that.
    - Layer names are unique across every controller, including blend-layer children, because drives such as `da.drive_switch("Hat")` refer to a layer by name alone.

#### Layers

- `da.group_layer(name, { driven_by, symmetric }, { da.default { ... }, da.option(name, targets), ... })`.
    - Option indices are assigned automatically during transform; declarations cannot specify them. References such as `da.drive_group(layer, option)` resolve the generated value from the layer and option. Use raw layers when parameter values are externally constrained.
    - Completion is always mutual-zeroed: the default absorbs the zeroed union of every option's keys (`ValueSet::union_fill_as_zero`), then each option inherits the default entries it lacks (`ValueSet::union_from_defaults`). There is no copy mode.
    - A non-zeroable entry (object reference) used by an option but missing from the default is an error.
    - `symmetric` chooses the state machine shape only; clip contents are the same either way.
        - `true` (default): every state is equal. Entry fans out to each option on `== index` and falls back to the default state, each option exits on `!= index`, the default state exits on each `== index`. Switching between options never passes through the default state, so its behaviors do not run on the way. If crossfades are added, they belong on exit transitions and apply to whatever state Entry resolves to, so they are per source state, not per destination. Generated durations are currently zero.
        - `false`: the v1 hub. The default state transitions to each option on `== index`, each option returns to the default state on `!= index`. Every switch passes through the default state for one frame and runs its behaviors; use this when that pass is wanted. Custom transition durations currently require a raw layer.
    - Group layers always generate state machines. For blend-tree output, write `da.raw.state` with `da.raw.blend_tree`: `type = "linear"` selects a 1D tree with a float axis, and `type = "direct"` selects fields with individual float weight parameters.
- `da.switch_layer(name, { driven_by }, enabled)` or `da.switch_layer(name, { driven_by }, disabled, enabled)`.
    - The options table is required (pass `{}` when empty) so that three arguments always mean a toggle list and four always mean both sides. Table shapes are never inspected to tell the forms apart.
    - The three-argument form is a toggle list: on gets the given or full value, off gets the zeroed value. Explicit `false` or zero values in a toggle list are an error.
    - The four-argument form spells both sides out in `disabled, enabled` order, matching `false, true`. An empty `disabled` list writes nothing in the off state, unlike a toggle list which writes zeroed values.
- `da.puppet_layer(name, { driven_by }, { da.keyframe(t, targets), ... })`.
    - Compiles to one state holding a 1D linear blend tree, not a motion-time clip: each keyframe becomes a fixed clip placed at threshold `t`, so keyframes are joined with linear interpolation. `driven_by` must be a float, and `t` is any real value rather than normalized time, so a `-1..1` puppet axis is used as is.
    - A target missing from a keyframe is filled by linearly interpolating its neighbours, and held at the ends. Every generated clip writes the same key set.
    - Step and Bezier interpolation are not exposed here. `da.raw.clip({ time_by }, targets)` still contains fixed targets; `time_by` controls playback and does not turn the targets into curves. Keyed curves currently exist only in the Rust animation model.
- `da.blend_layer(name, { da.puppet_layer(...), ... })` merges its children into one layer whose single state is a direct blend tree. Merging is explicit; layers written at the top level always stay separate.
    - Only layers that have no behaviors and are driven by a float can be merged, which is currently puppet layers only. Group and switch layers would need a float mirror of their parameter and are not accepted.
    - Each child becomes a direct field weighted by a float animator parameter fixed at `1.0`; the transform adds that parameter and it is not an expression parameter. The layer is Write Defaults on.
    - Children sum instead of overriding, so a target animated by two children of the same blend layer is an error. Overlap with other layers keeps the usual layer-order override.
    - The merged layer sits where the `da.blend_layer` is written in its controller. Drives such as `da.drive_puppet` keep referencing the child layer by name.

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
    controllers = {
        da.controller("fx", {
            da.group_layer("Expressions", { driven_by = "Emote" }, {
                da.default { Face:shape("eyelid_L", 0.3) },
                da.option("smile", { Face:shape("smile"), Face:shape("eye_joy", 0.5) }),
            }),
            hat and da.switch_layer("Hat", { driven_by = "Hat" }, { da.object("Hat"):active() }),
        }),
    },
    menu = {
        hat and da.toggle("Hat", da.drive_switch("Hat")),
    },
})
```

### Transform

- `transform::transform(&decl::Avatar) -> Result<avatar::Avatar, TransformErrors>`. `declavatar2::compile(source, chunk_name, &EvaluateOptions)` runs the interpreter and the transform in one call, returning `CompileError::Script` or `CompileError::Transform` on failure. This is the entry point the FFI wraps.
- The compiled model lives under `avatar`: `Avatar` holds the expression parameters, the `PlayableController`s, the menu and `Externals`. It is concrete over `Compiled`, as `decl` is over `Declared`. States and transitions refer to each other by index; a transition source is `Entry` or a state and a target is a state or `Exit`.
- A `PlayableController` is one `da.controller` in declaration order: its playable layer, `MergeMode`, priority, `PathMode`, the interned mask asset and an `AnimatorController`. An unwritten option takes its default there, not in the declaration. Every controller carries the whole animator parameter list, because a replaced Gesture layer still needs `GestureLeft` and merging by name is harmless.
- Errors are accumulated across parameter and layer collection, layer compilation and menu items. Within one failing layer or item, checking may stop at the first error; the remaining layers and items are still checked. Any error makes the whole transform fail, so a partial avatar is never returned. `TransformError` carries the `SourceLocation` of the reference that failed, falling back to the layer that holds it.
- Parameters
    - The animator parameter list follows the `parameters` block order, expanding each provided group in place, then appends generated parameters in compilation order. Expression parameters are the declared ones whose scope is not `internal`; the default scope is `synced` and `save` defaults to `false`. Unspecified bit widths stay `Unspecified`.
    - Provided parameters are declared with their VRChat names (`AFK`, `VRMode`, ...) and take part in type checks like any other parameter, so a group layer can be driven by `GestureLeft`.
    - A group, switch or puppet layer whose `driven_by` is omitted follows the parameter named after the layer. The parameter must exist: group requires int, switch requires bool, and puppet requires float. No driver parameter is generated implicitly.
    - A blend layer generates one float parameter `{blend}/{child}` per child, fixed at `1.0`. A collision with an existing parameter is an error.
- Layers
    - Group option indices are `1..n` in the written order, and the default state is index `0`. State names are `Default` and the option names.
    - A switch layer compiles to `Disabled` and `Enabled` with `Disabled` as the default state and one transition each way (`If` / `IfNot`). A toggle list whose entry cannot be zeroed (an object reference) is an error that asks for both sides.
    - A puppet layer sorts its keyframes by time and rejects two keyframes at the same time. A target written with different value types across keyframes is an error. Non-interpolable values (bool, int, object reference) that are missing from a keyframe take the previous written value.
    - Raw layer conditions compile per parameter type: `zero`/`nonzero`/`eq`/`ne` on bool and int, `gt`/`lt` on int and float, anything else is `UnsupportedCondition`. Written values go through `AnimatedValue::cast`, so an int literal against a float parameter is accepted. The default state is the written one, else the first state.
    - Clip options on the motion of a state become the state's `Playback` (`speed`, `speed_by`, `time_by`). Inside a blend tree only `speed` is meaningful and it becomes the field's speed; `speed_by` and `time_by` there are errors.
    - Group and switch transitions have duration `0.0`. Raw transitions preserve the written `duration`, defaulting to `0.0`.
    - Write Defaults is on only for the generated state of a blend layer; group, switch, puppet and raw states use off. The raw Lua API does not expose a Write Defaults option.
- Menu: a menu holds at most 8 controls. An axis accepts a float parameter name or `da.drive_puppet(layer)` without a value; any other drive on an axis is an error.
- Not done yet: the Unity client, Lua/declaration support for keyed curves, and bit width assignment for `Unspecified` widths. Gate/guard and exports were removed deliberately and are not pending features.

### Animation Model

- `InlineAnimation` distinguishes fixed clips (`ValueSet<FixedAnimationEntry>`) from keyed clips (`KeyedAnimation`). The declaration model and transform currently generate fixed clips only, including each field of a puppet blend tree.
- A `Curve` is non-empty and stores the first keyframe separately, then an interpolation and destination keyframe for each segment. Validation requires strictly increasing normalized times in `[0, 1]` and one value type throughout.
- Segment interpolation is `Constant`, `Linear` or `Bezier`. Constant accepts every value type; linear and Bezier require interpolable values. Bezier uses CSS-style control points with each x coordinate in `[0, 1]`.
- `ClipAttributes.length` maps normalized time to seconds. Loop time, loop blend and cycle offset are clip attributes; state playback speed and parameter-controlled playback are separate settings.

### Interop Format

- The FFI hands the client one byte blob per compile: a fixed positional format of our own, specified in [assets/interop-format.md](assets/interop-format.md). That document is the source of truth for the byte layout; this section records only the decisions behind it.
- A blob is a 16-byte header followed by a payload with no field names, no padding and no self-description beyond enum discriminators. The header carries a magic (`"DA2a"` for a compiled avatar, `"DA2d"` for diagnostics), a `schema_version` for the encoding rules and a `data_version` for the payload layout of that kind.
- A successful compile returns an avatar blob and a failed one returns a diagnostics blob. The two never share a call, so the magic alone tells them apart and no kind field exists.
- Versions are matched exactly on both sides. The native library and the client ship together and blobs are never persisted, so the versions exist to make a mismatch a clear error, not to keep old layouts readable. Any payload change, including a new enum variant, bumps `data_version`; any change to the encoding rules bumps `schema_version`. A golden-bytes test guards against bumping being forgotten.
- Rust side: a hand-written `Encode` / `Decode` per type in a `declavatar2::interop` module, with explicit discriminator constants. `serde` and `rmp-serde` are not dependencies.
- C# side: a hand-written reader mirroring the Rust encoder one to one. No serialization library dependency.
- Externals come first in the avatar payload so the client can resolve every table before reading the body that indexes into them.
- How the FFI hands the blob over is decided in the C FFI section.

### C FFI

- The `da2` crate is the C ABI. Its contract is the generated header `da2/include/da2.h` plus the rules in [assets/c-ffi.md](assets/c-ffi.md); this section records only the decisions.
- One owned object only: `Da2Context` holds the symbols, the library paths and the last result blob, and `da2_context_free` is the only release call. The blob is not a separate handle. `da2_context_result` borrows it from the context until the next compile, reset or free, and the client copies it out at once.
- `da2_context_compile` returns `Success` with an avatar blob or `CompileFailed` with a diagnostics blob; every other status stores no result. There is no last-error string, because the remaining failures are caller mistakes or library faults that a status already names.
- Strings cross as `Da2Str`, a pointer and a byte length of UTF-8 without a terminator. A null pointer with zero length is the empty string, since a pinned empty array is null in C#.
- Every export catches panics and returns `Panicked` rather than aborting the host.
- A context is not thread-safe and contexts are independent. `EvaluateOptions` holds `Rc` loaders, so the context is not `Send`, and nothing needs it to be.
- The header is generated by cbindgen from `build.rs` behind the `bindings` feature and checked in; CI fails when it is stale. The C# declarations are hand-written in the client, like the interop reader.
- Windows links the C runtime statically through `.cargo/config.toml`, and the release profile uses fat LTO. `build-da2.yml` builds Windows x86_64, Linux x86_64 on `ubuntu-22.04` for glibc compatibility, and a universal macOS dylib.

### External References

- v1 took a `Map<String, Asset>` ScriptableObject as compiler input. v2 reverses that: compilation needs nothing but the script and the symbols, and the resulting avatar enumerates what it requires.
- `Externals` is that enumeration. Object paths, component types and assets each get an `ExternTable`, deduplicated, and the avatar body refers to them by index. Every entry carries `referenced_at`, so an unmet requirement is reported against the line that asked for it.
- `Externals.needs_relative_root` says whether any controller uses `PathMode::Relative`. It sits next to the tables because it is the same kind of request: something the client has to supply (the root object) before it can apply the avatar.
- The client resolves the tables in index order and builds one array per kind, then reads the avatar body straight through it. No second compilation pass is involved. An object path is resolved against the avatar root or, for a relative controller, against the supplied root, so the same table entry may be looked up under both.
- Resolving an asset is the client's job, as is checking that what it found matches the `asset_type` of a `Named` locator.
- A dictionary ScriptableObject equivalent to v1's is still accepted, as one source of resolutions rather than a compiler input. For `AssetLocator::Named` the dictionary wins, and a project-wide search is used only when it hits exactly one asset.
- Losing compile-time name checking is the deliberate cost. Reporting a failure at the right script line keeps it manageable.

## Code Style

- Rust edition 2024
- Avoid comments by default (add comments only when explicitly requested by the user).
- Keep comparisons with declavatar v1 and migration rationale in this document only. Code, API documentation and test names should describe current behavior without historical comparisons.
- Utilize rstest features.

## Build & Test

- Rust workspace layout
- Build and test with `cargo build` and `cargo test`
- lint/typecheck: `cargo clippy`, `cargo fmt --check`
