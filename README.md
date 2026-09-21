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
