# Declavatar 2

Declarative Avatar Asset Composing Tool; Revised

Declavatar 2 compiles a Lua script into VRChat avatar data: Expression Parameters, animator controllers for every playable layer and the Expression Menu. It is a redesign of [declavatar](https://github.com/kb10uy/declavatar) with a Rust core, a C FFI and a byte-level interop format that any client can read.

## Packages

| Crate | Contents |
|---|---|
| `declavatar2` | The library: Lua interpreter, declaration model, transform and interop encoder. |
| `da2` | The C ABI (`da2.dll`, `libda2.so`, `libda2.dylib`) that wraps `declavatar2::compile`. |

## A Script

```lua
local da = require "declavatar"

local Face = da.renderer("Face")
local hat = da.symbol("ENABLE_HAT")

return da.avatar({
    parameters = {
        da.provided("VRChat"),
        da.int("Emote", { default = 0 }),
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

A script requires `declavatar` and returns exactly one `da.avatar(...)`. Builders validate their arguments at the call site, and the transform reports every unresolved reference with the line of the script that wrote it. `da.symbol` asks whether the client supplied a symbol, so conditional content is plain Lua control flow.

Editor completion comes from the lua-language-server definitions under `declavatar2/lua/types`; see [declavatar2/lua/README.md](declavatar2/lua/README.md).

## Design

Declavatar 2 is a translator, not a framework. It turns a script into the data Unity and VRChat already define, as literally as it can, and leaves everything else to the tools that own it. Two decisions shape the rest: Unity's data model is the data model, and nothing is optimized.

### The script

**Lua is the language.** Repetition, conditions and abstraction are written with Lua functions and control flow; the host contributes only builders that return immutable nodes. Helpers live in Lua modules such as `declavatar.ext`, not in the host.

**Nothing is generated implicitly.** A layer whose driving parameter does not exist is an error, not a declaration. Nested lists are not flattened. Layers are merged only by `da.blend_layer`. An unknown key in an options table is an error, so a typo cannot become a silent default.

**Errors point at the line.** Errors of shape and type fail at the builder call with a Lua traceback, and errors of reference and consistency fail in the transform, each with the chunk and line of the script that wrote them.

### The data model

**Unity's data model is the data model.** The compiled avatar mirrors `AnimatorController`, Expression Parameters and the Expression Menu one to one, under the names Unity gives them. Group, switch and puppet layers are sugar with a documented lowering, and what they cannot express is written as a raw layer against the same model. Nothing is accepted that the model cannot hold.

**Typed where the transform must understand, verbatim elsewhere.** Drives, tracking control and the VRChat state behaviors are typed so that they can be checked. Any other behavior goes through `da.behavior` as plain values that the transform passes along untouched.

### The compiler

**No optimization.** The output is a literal translation of the script: equal clips are not shared, layers are not merged unless the script merges them, and parameters are not packed. Under NDMF that work belongs to the plugins of the Optimizing phase such as AAO Avatar Optimizer, and doing it here would fight them over the result.

**Compilation is a pure function of the script and the symbols.** No project data is passed in and a script cannot reach the filesystem. Instead the compiled avatar enumerates every object path, component type and asset it requires, and the client resolves them against the project.

**The side that knows decides.** Animator layer indices are known only to the client, so a layer control names a layer. Bit widths of expression parameters stay unspecified so that Modular Avatar and NDMF assign them. Settings that describe the project rather than the avatar are not written in a script.

**No partial output.** Errors are collected across every layer and menu item, and one error fails the whole compile. A broken avatar never reaches Unity.

**Hand-written interfaces, few dependencies.** The interop blob is positional and matched by exact version on both sides, with the encoder and the reader written by hand in Rust and in C#. The C ABI is one owned context, and every export catches panics.

### Non-goals

- **Optimization.** Clip sharing, parameter packing and layer merging belong to optimizer plugins.
- **Compatibility with declavatar v1.** The script language and the data model were redesigned, and v1 scripts do not run as they are.
- **Implicit completion.** Undeclared parameters, auto-flattened lists, a layer-level default mesh and guessed asset types were removed deliberately.
- **Host features that Lua covers.** Gate and guard, exports and `tracking_layer` are not pending features; write them in Lua.
- **Reaching into the Unity project from a script.** A script sees no filesystem and no project. The client supplies what the avatar enumerates.
- **Protecting against runaway scripts.** No memory or instruction limit is imposed. A script that loops forever hangs the caller, at its own risk.

## Building

Rust 1.89 or later is required.

```
cargo build -p da2 --release
```

produces the shared library under `target/release`. The release profile uses fat LTO, and on Windows the C runtime is linked statically, so the DLL needs no redistributable. Tagged releases on GitHub carry prebuilt binaries for Windows x86_64, Linux x86_64 and a universal macOS build together with `da2.h`.

`cargo test` runs the whole suite, including the interop round trips and the golden blobs under `declavatar2/tests/golden`. After changing the FFI surface, regenerate the header with `cargo build -p da2 --features bindings`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The design decisions behind the Lua API, the transform and the interop format are recorded in [AGENTS.md](AGENTS.md).

## License

SPDX: `Apache-2.0 OR MIT`
