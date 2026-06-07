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

- Scripts should end with `return da.avatar(...)` and return exactly one avatar (like Lua module convention).
- Unlike declavatar v1, declavatar2 does not auto-flatten nested lists.
    - Provide `da.concat` as a composition utility instead.

```lua
local da = require "declavatar";

return da.avatar("name", {
    parameters = {
        da.int("Emote", { default = 42 }),
        da.bool("Hat", { scope = "local" }),
    },
    fx_controller = {
        da.group_layer("Expressions", { driven_by = "Emote" }, {
            da.option("smile", { ... }),
        }),
    },
    menu = {
        da.toggle("Hat", da.drive_switch("Hat")),
    },
});
```

### Interop Format

- Use **MessagePack** instead of JSON used in v1.
- Rust side: `rmp-serde` (direct output from serde `Serialize`).
- C# side: `MessagePack-CSharp` (with Source Generator support).
- Simplify the C FFI API to a single compile -> MessagePack blob flow.

## Code Style

- Avoid comments by default (add comments only when explicitly requested by the user).
- Rust edition 2024

## Build & Test

- Rust workspace layout
- Build and test with `cargo build` and `cargo test`
- lint/typecheck: `cargo clippy`, `cargo fmt --check`
