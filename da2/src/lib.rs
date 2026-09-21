use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice, str,
};

use declavatar2::{
    EvaluateOptions, compile,
    interop::{AVATAR_DATA_VERSION, DIAGNOSTICS_DATA_VERSION, Diagnostics, SCHEMA_VERSION, encode_avatar, encode_diagnostics},
};

/// Outcome of a call. Values below 100 are ordinary outcomes, 100 to 199 are caller mistakes and 200 and above are faults of the library.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Da2Status {
    /// The call succeeded. After `da2_context_compile`, the result is an avatar blob.
    Success = 0,

    /// The script was rejected. The result is a diagnostics blob.
    CompileFailed = 1,

    /// A required pointer was null.
    InvalidPointer = 100,

    /// A string was not valid UTF-8.
    InvalidUtf8 = 101,

    /// The context holds no result because nothing has been compiled since it was created or reset.
    NoResult = 102,

    /// The compiled data could not be encoded into a blob.
    EncodeFailed = 200,

    /// The library panicked. The context may be left inconsistent and should be freed.
    Panicked = 201,
}

/// Borrowed UTF-8 text without a terminator. A null pointer with zero length is the empty string.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Da2Str {
    pub ptr: *const u8,
    pub len: u32,
}

/// The interop format versions this library writes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Da2FormatVersions {
    pub schema: u16,
    pub avatar_data: u16,
    pub diagnostics_data: u16,
}

/// Everything the library keeps for a caller: the compile options and the last result.
///
/// A context is not thread-safe. Different contexts may be used from different threads at the same time.
pub struct Da2Context {
    options: EvaluateOptions,
    result: Option<Vec<u8>>,
}

fn guarded(body: impl FnOnce() -> Da2Status) -> Da2Status {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(Da2Status::Panicked)
}

unsafe fn context_mut<'a>(context: *mut Da2Context) -> Result<&'a mut Da2Context, Da2Status> {
    unsafe { context.as_mut() }.ok_or(Da2Status::InvalidPointer)
}

unsafe fn context_ref<'a>(context: *const Da2Context) -> Result<&'a Da2Context, Da2Status> {
    unsafe { context.as_ref() }.ok_or(Da2Status::InvalidPointer)
}

unsafe fn text<'a>(text: Da2Str) -> Result<&'a str, Da2Status> {
    if text.ptr.is_null() {
        return if text.len == 0 { Ok("") } else { Err(Da2Status::InvalidPointer) };
    }
    let bytes = unsafe { slice::from_raw_parts(text.ptr, text.len as usize) };
    str::from_utf8(bytes).map_err(|_| Da2Status::InvalidUtf8)
}

fn status(result: Result<(), Da2Status>) -> Da2Status {
    match result {
        Ok(()) => Da2Status::Success,
        Err(status) => status,
    }
}

/// Reports the interop format versions, so that a client can refuse a mismatched library before compiling anything.
#[unsafe(no_mangle)]
pub extern "C" fn da2_format_versions() -> Da2FormatVersions {
    Da2FormatVersions {
        schema: SCHEMA_VERSION,
        avatar_data: AVATAR_DATA_VERSION,
        diagnostics_data: DIAGNOSTICS_DATA_VERSION,
    }
}

/// Creates a context with no symbols, no library paths and no result. Release it with `da2_context_free`.
#[unsafe(no_mangle)]
pub extern "C" fn da2_context_new() -> *mut Da2Context {
    Box::into_raw(Box::new(Da2Context {
        options: EvaluateOptions::new(),
        result: None,
    }))
}

/// Releases a context and the result it holds. A null pointer is ignored.
///
/// # Safety
/// `context` must have come from `da2_context_new` and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn da2_context_free(context: *mut Da2Context) {
    if !context.is_null() {
        drop(unsafe { Box::from_raw(context) });
    }
}

/// Makes `da.symbol(name)` report the symbol as supplied in later compiles.
///
/// # Safety
/// `context` must be a live context and `name` must point at `name.len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn da2_context_add_symbol(context: *mut Da2Context, name: Da2Str) -> Da2Status {
    guarded(|| {
        status((|| {
            let context = unsafe { context_mut(context) }?;
            let name = unsafe { text(name) }?;
            context.options = std::mem::take(&mut context.options).symbols([name]);
            Ok(())
        })())
    })
}

/// Appends a directory that `require` searches in later compiles, after the ones already added.
///
/// # Safety
/// `context` must be a live context and `path` must point at `path.len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn da2_context_add_library_path(context: *mut Da2Context, path: Da2Str) -> Da2Status {
    guarded(|| {
        status((|| {
            let context = unsafe { context_mut(context) }?;
            let path = unsafe { text(path) }?;
            context.options = std::mem::take(&mut context.options).library_paths([path]);
            Ok(())
        })())
    })
}

/// Forgets every symbol, library path and result, returning the context to its initial state.
///
/// # Safety
/// `context` must be a live context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn da2_context_reset(context: *mut Da2Context) -> Da2Status {
    guarded(|| {
        status((|| {
            let context = unsafe { context_mut(context) }?;
            context.options = EvaluateOptions::new();
            context.result = None;
            Ok(())
        })())
    })
}

/// Compiles a script and stores the resulting blob in the context, replacing any previous result.
///
/// `chunk_name` is how the script is named in diagnostics; Lua shortens long names, so a file name serves better than a full path.
/// Returns `Success` with an avatar blob, `CompileFailed` with a diagnostics blob, or another status with no result.
///
/// # Safety
/// `context` must be a live context, and `source` and `chunk_name` must point at their declared number of readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn da2_context_compile(context: *mut Da2Context, source: Da2Str, chunk_name: Da2Str) -> Da2Status {
    guarded(|| {
        let (context, source, chunk_name) = match (|| {
            let context = unsafe { context_mut(context) }?;
            let source = unsafe { text(source) }?;
            let chunk_name = unsafe { text(chunk_name) }?;
            Ok((context, source, chunk_name))
        })() {
            Ok(arguments) => arguments,
            Err(status) => return status,
        };

        context.result = None;
        let encoded = match compile(source, chunk_name, &context.options) {
            Ok(avatar) => encode_avatar(&avatar).map(|bytes| (bytes, Da2Status::Success)),
            Err(error) => encode_diagnostics(&Diagnostics::from(&error)).map(|bytes| (bytes, Da2Status::CompileFailed)),
        };
        match encoded {
            Ok((bytes, status)) if u32::try_from(bytes.len()).is_ok() => {
                context.result = Some(bytes);
                status
            }
            _ => Da2Status::EncodeFailed,
        }
    })
}

/// Exposes the blob stored by the last `da2_context_compile`.
///
/// The bytes belong to the context and stay valid until the next compile, reset or free. Copy them before that.
///
/// # Safety
/// `context` must be a live context, and `ptr` and `len` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn da2_context_result(context: *const Da2Context, ptr: *mut *const u8, len: *mut u32) -> Da2Status {
    guarded(|| {
        status((|| {
            let context = unsafe { context_ref(context) }?;
            if ptr.is_null() || len.is_null() {
                return Err(Da2Status::InvalidPointer);
            }
            let result = context.result.as_ref().ok_or(Da2Status::NoResult)?;
            unsafe {
                ptr::write(ptr, result.as_ptr());
                ptr::write(len, result.len() as u32);
            }
            Ok(())
        })())
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use declavatar2::interop::{DiagnosticStage, decode_avatar, decode_diagnostics};
    use rstest::*;

    use super::*;

    const SCRIPT: &str = r#"local da = require "declavatar"

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
"#;

    struct Context(*mut Da2Context);

    impl Context {
        fn new() -> Self {
            Self(da2_context_new())
        }

        fn add_symbol(&self, name: &str) -> Da2Status {
            unsafe { da2_context_add_symbol(self.0, text(name)) }
        }

        fn add_library_path(&self, path: &str) -> Da2Status {
            unsafe { da2_context_add_library_path(self.0, text(path)) }
        }

        fn reset(&self) -> Da2Status {
            unsafe { da2_context_reset(self.0) }
        }

        fn compile(&self, source: &str) -> Da2Status {
            unsafe { da2_context_compile(self.0, text(source), text("avatar.lua")) }
        }

        fn result(&self) -> Result<Vec<u8>, Da2Status> {
            let mut ptr = ptr::null();
            let mut len = 0;
            match unsafe { da2_context_result(self.0, &mut ptr, &mut len) } {
                Da2Status::Success => Ok(unsafe { slice::from_raw_parts(ptr, len as usize) }.to_vec()),
                status => Err(status),
            }
        }
    }

    impl Drop for Context {
        fn drop(&mut self) {
            unsafe { da2_context_free(self.0) }
        }
    }

    fn text(text: &str) -> Da2Str {
        Da2Str {
            ptr: text.as_ptr(),
            len: text.len() as u32,
        }
    }

    fn layer_names(blob: &[u8]) -> Vec<String> {
        let avatar = decode_avatar(blob).expect("the result should be an avatar blob");
        avatar
            .controllers
            .iter()
            .flat_map(|c| c.controller.layers.iter().map(|l| l.name.clone()))
            .collect()
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};

            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!("da2-modules-{}-{unique}", std::process::id()));
            fs::create_dir_all(&root).expect("temporary directory should be created");
            Self(root)
        }

        fn write(&self, relative: &str, contents: &str) {
            fs::write(self.0.join(relative), contents).expect("file should be written");
        }

        fn path(&self) -> &str {
            self.0.to_str().expect("temporary path should be UTF-8")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[rstest]
    fn versions_are_the_ones_the_encoder_writes() {
        assert_eq!(
            da2_format_versions(),
            Da2FormatVersions {
                schema: SCHEMA_VERSION,
                avatar_data: AVATAR_DATA_VERSION,
                diagnostics_data: DIAGNOSTICS_DATA_VERSION,
            }
        );
    }

    #[rstest]
    fn a_compiled_script_yields_an_avatar_blob() {
        let context = Context::new();
        assert_eq!(context.compile(SCRIPT), Da2Status::Success);

        let blob = context.result().expect("a result should be stored");
        assert_eq!(&blob[..4], b"DA2a");
        assert_eq!(layer_names(&blob), ["Expressions"]);
    }

    #[rstest]
    fn symbols_reach_the_script() {
        let context = Context::new();
        assert_eq!(context.add_symbol("ENABLE_HAT"), Da2Status::Success);
        assert_eq!(context.compile(SCRIPT), Da2Status::Success);

        assert_eq!(layer_names(&context.result().unwrap()), ["Expressions", "Hat"]);
    }

    #[rstest]
    fn a_script_error_yields_a_diagnostics_blob() {
        let context = Context::new();
        assert_eq!(context.compile("local da = require 'declavatar'\nerror('boom')\n"), Da2Status::CompileFailed);

        let blob = context.result().expect("a result should be stored");
        assert_eq!(&blob[..4], b"DA2d");
        let diagnostics = decode_diagnostics(&blob).expect("the result should be a diagnostics blob");
        assert_eq!(diagnostics.stage, DiagnosticStage::Script);
        assert_eq!(diagnostics.items.len(), 1);
        assert!(diagnostics.items[0].message.contains("avatar.lua:2: boom"), "{}", diagnostics.items[0].message);
    }

    #[rstest]
    fn a_transform_error_yields_located_diagnostics() {
        let context = Context::new();
        let source = "local da = require 'declavatar'\nreturn da.avatar({ controllers = { da.controller('fx', {\n  da.switch_layer('Hat', {}, { da.object('Hat'):active() }),\n}) } })\n";
        assert_eq!(context.compile(source), Da2Status::CompileFailed);

        let diagnostics = decode_diagnostics(&context.result().unwrap()).unwrap();
        assert_eq!(diagnostics.stage, DiagnosticStage::Transform);
        let item = &diagnostics.items[0];
        assert_eq!(item.at.as_ref().map(|at| (at.chunk.as_str(), at.line)), Some(("avatar.lua", 3)));
        assert!(item.message.contains("parameter `Hat` is not declared"), "{}", item.message);
    }

    #[rstest]
    fn library_paths_serve_modules_to_require() {
        let modules = TempDir::new();
        modules.write("hat.lua", "return { name = 'Hat' }");
        let source = "local da = require 'declavatar'\nlocal hat = require 'hat'\nreturn da.avatar({ parameters = { da.bool(hat.name) } })\n";

        let context = Context::new();
        assert_eq!(context.compile(source), Da2Status::CompileFailed);
        assert_eq!(context.add_library_path(modules.path()), Da2Status::Success);
        assert_eq!(context.compile(source), Da2Status::Success);

        let avatar = decode_avatar(&context.result().unwrap()).unwrap();
        assert_eq!(avatar.expression_parameters[0].name, "Hat");
    }

    #[rstest]
    fn the_result_is_absent_until_a_compile_and_after_a_reset() {
        let context = Context::new();
        assert_eq!(context.result().unwrap_err(), Da2Status::NoResult);

        context.add_symbol("ENABLE_HAT");
        assert_eq!(context.compile(SCRIPT), Da2Status::Success);
        assert!(context.result().is_ok());

        assert_eq!(context.reset(), Da2Status::Success);
        assert_eq!(context.result().unwrap_err(), Da2Status::NoResult);

        assert_eq!(context.compile(SCRIPT), Da2Status::Success);
        assert_eq!(layer_names(&context.result().unwrap()), ["Expressions"]);
    }

    #[rstest]
    fn each_compile_replaces_the_previous_result() {
        let context = Context::new();
        assert_eq!(context.compile(SCRIPT), Da2Status::Success);
        assert_eq!(context.compile("return 1"), Da2Status::CompileFailed);
        assert_eq!(&context.result().unwrap()[..4], b"DA2d");
    }

    #[rstest]
    fn an_empty_string_may_be_a_null_pointer() {
        let context = Context::new();
        let empty = Da2Str { ptr: ptr::null(), len: 0 };
        assert_eq!(unsafe { da2_context_compile(context.0, empty, empty) }, Da2Status::CompileFailed);

        let diagnostics = decode_diagnostics(&context.result().unwrap()).unwrap();
        assert!(diagnostics.items[0].message.contains("returned nothing"), "{}", diagnostics.items[0].message);
    }

    #[rstest]
    fn a_null_string_with_a_length_is_rejected() {
        let context = Context::new();
        let dangling = Da2Str { ptr: ptr::null(), len: 3 };
        assert_eq!(unsafe { da2_context_add_symbol(context.0, dangling) }, Da2Status::InvalidPointer);
        assert_eq!(
            unsafe { da2_context_compile(context.0, dangling, text("avatar.lua")) },
            Da2Status::InvalidPointer
        );
        assert_eq!(context.result().unwrap_err(), Da2Status::NoResult);
    }

    #[rstest]
    fn invalid_utf8_is_rejected() {
        let context = Context::new();
        let bytes = [0xff, 0xfe];
        let invalid = Da2Str {
            ptr: bytes.as_ptr(),
            len: bytes.len() as u32,
        };
        assert_eq!(unsafe { da2_context_add_symbol(context.0, invalid) }, Da2Status::InvalidUtf8);
        assert_eq!(unsafe { da2_context_add_library_path(context.0, invalid) }, Da2Status::InvalidUtf8);
        assert_eq!(unsafe { da2_context_compile(context.0, invalid, text("avatar.lua")) }, Da2Status::InvalidUtf8);
        assert_eq!(unsafe { da2_context_compile(context.0, text(SCRIPT), invalid) }, Da2Status::InvalidUtf8);
    }

    #[rstest]
    fn a_null_context_is_rejected_by_every_call() {
        let null = ptr::null_mut();
        let mut ptr = ptr::null();
        let mut len = 0;
        assert_eq!(unsafe { da2_context_add_symbol(null, text("A")) }, Da2Status::InvalidPointer);
        assert_eq!(unsafe { da2_context_add_library_path(null, text(".")) }, Da2Status::InvalidPointer);
        assert_eq!(unsafe { da2_context_reset(null) }, Da2Status::InvalidPointer);
        assert_eq!(
            unsafe { da2_context_compile(null, text(SCRIPT), text("avatar.lua")) },
            Da2Status::InvalidPointer
        );
        assert_eq!(unsafe { da2_context_result(null, &mut ptr, &mut len) }, Da2Status::InvalidPointer);
        unsafe { da2_context_free(null) };
    }

    #[rstest]
    fn result_needs_both_output_pointers() {
        let context = Context::new();
        assert_eq!(context.compile(SCRIPT), Da2Status::Success);

        let mut ptr = ptr::null();
        let mut len = 0;
        assert_eq!(unsafe { da2_context_result(context.0, ptr::null_mut(), &mut len) }, Da2Status::InvalidPointer);
        assert_eq!(unsafe { da2_context_result(context.0, &mut ptr, ptr::null_mut()) }, Da2Status::InvalidPointer);
    }
}
