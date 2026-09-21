#ifndef DA2_H
#define DA2_H

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Outcome of a call. Values below 100 are ordinary outcomes, 100 to 199 are caller mistakes and 200 and above are faults of the library.
 */
enum Da2Status
#if defined(__cplusplus) || __STDC_VERSION__ >= 202311L
  : uint32_t
#endif // defined(__cplusplus) || __STDC_VERSION__ >= 202311L
 {
  /**
   * The call succeeded. After `da2_context_compile`, the result is an avatar blob.
   */
  DA2_STATUS_SUCCESS = 0,
  /**
   * The script was rejected. The result is a diagnostics blob.
   */
  DA2_STATUS_COMPILE_FAILED = 1,
  /**
   * A required pointer was null.
   */
  DA2_STATUS_INVALID_POINTER = 100,
  /**
   * A string was not valid UTF-8.
   */
  DA2_STATUS_INVALID_UTF8 = 101,
  /**
   * The context holds no result because nothing has been compiled since it was created or reset.
   */
  DA2_STATUS_NO_RESULT = 102,
  /**
   * The compiled data could not be encoded into a blob.
   */
  DA2_STATUS_ENCODE_FAILED = 200,
  /**
   * The library panicked. The context may be left inconsistent and should be freed.
   */
  DA2_STATUS_PANICKED = 201,
};
#ifndef __cplusplus
#if __STDC_VERSION__ >= 202311L
typedef enum Da2Status Da2Status;
#else
typedef uint32_t Da2Status;
#endif // __STDC_VERSION__ >= 202311L
#endif // __cplusplus

/**
 * Everything the library keeps for a caller: the compile options and the last result.
 *
 * A context is not thread-safe. Different contexts may be used from different threads at the same time.
 */
typedef struct Da2Context Da2Context;

/**
 * The interop format versions this library writes.
 */
typedef struct Da2FormatVersions {
  uint16_t schema;
  uint16_t avatar_data;
  uint16_t diagnostics_data;
} Da2FormatVersions;

/**
 * Borrowed UTF-8 text without a terminator. A null pointer with zero length is the empty string.
 */
typedef struct Da2Str {
  const uint8_t *ptr;
  uint32_t len;
} Da2Str;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Reports the interop format versions, so that a client can refuse a mismatched library before compiling anything.
 */
struct Da2FormatVersions da2_format_versions(void);

/**
 * Creates a context with no symbols, no library paths and no result. Release it with `da2_context_free`.
 */
struct Da2Context *da2_context_new(void);

/**
 * Releases a context and the result it holds. A null pointer is ignored.
 *
 * # Safety
 * `context` must have come from `da2_context_new` and must not be used afterwards.
 */
void da2_context_free(struct Da2Context *context);

/**
 * Makes `da.symbol(name)` report the symbol as supplied in later compiles.
 *
 * # Safety
 * `context` must be a live context and `name` must point at `name.len` readable bytes.
 */
Da2Status da2_context_add_symbol(struct Da2Context *context, struct Da2Str name);

/**
 * Appends a directory that `require` searches in later compiles, after the ones already added.
 *
 * # Safety
 * `context` must be a live context and `path` must point at `path.len` readable bytes.
 */
Da2Status da2_context_add_library_path(struct Da2Context *context, struct Da2Str path);

/**
 * Forgets every symbol, library path and result, returning the context to its initial state.
 *
 * # Safety
 * `context` must be a live context.
 */
Da2Status da2_context_reset(struct Da2Context *context);

/**
 * Compiles a script and stores the resulting blob in the context, replacing any previous result.
 *
 * `chunk_name` is how the script is named in diagnostics; Lua shortens long names, so a file name serves better than a full path.
 * Returns `Success` with an avatar blob, `CompileFailed` with a diagnostics blob, or another status with no result.
 *
 * # Safety
 * `context` must be a live context, and `source` and `chunk_name` must point at their declared number of readable bytes.
 */
Da2Status da2_context_compile(struct Da2Context *context,
                              struct Da2Str source,
                              struct Da2Str chunk_name);

/**
 * Exposes the blob stored by the last `da2_context_compile`.
 *
 * The bytes belong to the context and stay valid until the next compile, reset or free. Copy them before that.
 *
 * # Safety
 * `context` must be a live context, and `ptr` and `len` must be writable.
 */
Da2Status da2_context_result(const struct Da2Context *context,
                             const uint8_t **ptr,
                             uint32_t *len);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* DA2_H */
