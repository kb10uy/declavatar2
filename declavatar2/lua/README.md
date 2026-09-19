# Lua side of declavatar2

## `runtime/`

Lua that declavatar2 actually runs. `ext.lua` is embedded into the binary with
`include_str!` and preloaded as `declavatar.ext`, so editing it changes the shipped
module and a rebuild is needed to see the change.

## `types/`

Definition files for [lua-language-server](https://github.com/LuaLS/lua-language-server).
They carry annotations only and are never executed, so nothing here affects a build.
`declavatar` is implemented in Rust and `declavatar.ext` in `runtime/`; both are
described here so that an editor can complete and check a declaration script.

Point the language server at the directory to pick them up. In `.luarc.json`:

```json
{
  "runtime": { "version": "Lua 5.4" },
  "workspace": { "library": ["path/to/declavatar2/lua/types"] }
}
```

The repository root has one already, so the scripts under this directory get the same
completion that a user's script does.

A test compares these definitions against the functions the runtime really registers, so
a builder added on the Rust side without a matching annotation fails the build.
