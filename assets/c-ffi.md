# C FFI

The `da2` crate is the C ABI of declavatar2, built as `da2.dll`, `libda2.so` and `libda2.dylib`. It wraps `declavatar2::compile` and the interop encoder and nothing else; the client reads the blob it hands back with the reader described in [interop-format.md](interop-format.md). The generated header at `da2/include/da2.h` is the contract; this document records the rules the header cannot express.

## One Object to Free

The only thing a caller ever allocates or frees is a `Da2Context`, created by `da2_context_new` and released by `da2_context_free`. A context holds the compile options (symbols and library paths) and the blob produced by the last compile. Nothing else crosses the boundary as an owned object: strings are borrowed for the duration of a call, and the result blob is borrowed from the context.

A typical client wraps the context in a disposable handle and does the whole job inside it:

```
context = da2_context_new()
da2_context_add_symbol(context, "ENABLE_HAT")
da2_context_add_library_path(context, "Packages/…/Lua")
status = da2_context_compile(context, source, "avatar.lua")
da2_context_result(context, &ptr, &len)     // copy len bytes out of ptr
da2_context_free(context)
```

## Strings

Every string parameter is a `Da2Str`: a pointer and a byte length of UTF-8 text, with no terminator. The library reads the bytes during the call only and never keeps the pointer. A null pointer with a zero length is the empty string, because a pinned empty buffer is null in some runtimes; a null pointer with a non-zero length is `DA2_STATUS_INVALID_POINTER`. Bytes that are not valid UTF-8 are `DA2_STATUS_INVALID_UTF8` and nothing is changed.

`chunk_name` is the name diagnostics use for the script. Lua truncates long chunk names in tracebacks and source locations, so a file name serves better than a full path.

## Results

`da2_context_compile` returns `DA2_STATUS_SUCCESS` and stores an avatar blob (`DA2a`), or `DA2_STATUS_COMPILE_FAILED` and stores a diagnostics blob (`DA2d`). Any other status stores nothing. Every compile discards the previous result first, so a context holds at most one blob.

`da2_context_result` exposes that blob as a pointer and a length. The bytes belong to the context and stay valid until the next `da2_context_compile`, `da2_context_reset` or `da2_context_free`; a client copies them into its own buffer before doing any of those. Before the first compile, and after a reset, the status is `DA2_STATUS_NO_RESULT`.

The blob header is self-identifying, and the client is expected to validate it as the interop document describes regardless of the status it received. `da2_format_versions` reports the versions the library writes, so a client can refuse a mismatched library up front with a clear message rather than after a compile.

## Statuses

| Value | Name | Meaning |
|---|---|---|
| 0 | `SUCCESS` | The call did what it says. |
| 1 | `COMPILE_FAILED` | The script was rejected; the result is a diagnostics blob. |
| 100 | `INVALID_POINTER` | A required pointer was null. |
| 101 | `INVALID_UTF8` | A string argument was not UTF-8. |
| 102 | `NO_RESULT` | Nothing has been compiled since the context was created or reset. |
| 200 | `ENCODE_FAILED` | The compiled data could not be encoded; this is a bug in the library. |
| 201 | `PANICKED` | The library panicked; free the context and report the bug. |

Values below 100 are ordinary outcomes, 100 to 199 are caller mistakes and 200 and above are faults of the library. There is no last-error string: the only failures that do not produce a diagnostics blob are the ones a status already names.

## Threads and Panics

A context is not thread-safe and must not be shared between threads without external locking. Contexts are independent of each other, so different threads may compile with different contexts at the same time; each compile runs in a fresh Lua state.

Every exported function catches Rust panics and turns them into `DA2_STATUS_PANICKED` instead of aborting the host process. A context that reported a panic may be inconsistent and should be freed.

## Building

`cargo build -p da2 --release` produces the library. The workspace release profile enables fat LTO and strips debug info; on Windows the C runtime is linked statically through `.cargo/config.toml`, so the DLL needs no redistributable.

The header is regenerated with `cargo build -p da2 --features bindings`, which runs cbindgen from `build.rs` and overwrites `da2/include/da2.h`. Regenerate it in the same change as any signature or documentation change. The C# declarations in the Unity client are written by hand from the header, which is small enough that a generator would not pay for itself.
