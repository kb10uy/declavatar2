# Interop Format

The C FFI (`da2`) hands the client one byte blob per call. This document defines that blob: a fixed 16-byte header followed by a positional payload whose layout is known to both sides in advance. Nothing in the payload is self-describing; there are no field names, no per-value type tags beyond enum discriminators, and no padding.

The format is written by the Rust core and read by the client (C#/Unity). The Rust side also implements a reader so that round trips are tested with `cargo test`.

Two blobs exist and never share a call: a successful compile returns an **Avatar** blob, a failed compile returns a **Diagnostics** blob. They share the header layout and the encoding rules, and are told apart by magic.

## Header

Every blob starts with 16 bytes. All multi-byte integers in the whole blob are little-endian.

| Offset | Size | Field | Contents |
|---|---|---|---|
| 0 | 4 | `magic` | `"DA2a"` (`44 41 32 61`) for Avatar, `"DA2d"` (`44 41 32 64`) for Diagnostics |
| 4 | 2 | `schema_version` | u16. Version of the encoding rules below. Currently `1`. |
| 6 | 2 | `data_version` | u16. Version of the payload layout of this blob kind. Currently `1` for both kinds. |
| 8 | 4 | `reserved` | Zero. |
| 12 | 4 | `payload_len` | u32. Number of bytes following the header. |
| 16 | `payload_len` | `payload` | See the payload sections. |

A reader validates in this order and fails with a distinct message at each step:

1. The blob contains at least 16 bytes; otherwise the header is truncated.
2. `magic` is one of the two known values; otherwise the bytes are not a declavatar blob.
3. `schema_version` equals the reader's constant; otherwise the native library and the client do not match.
4. `data_version` equals the reader's constant for that magic; same failure as above.
5. `reserved` is zero.
6. The blob length passed across the FFI equals `16 + payload_len`, calculated without integer overflow.

The payload starts at offset 16 and is byte-packed; no field is aligned.

## Encoding Rules

These rules are what `schema_version` covers. Changing any of them bumps `schema_version`.

| Notation | Encoding |
|---|---|
| `u8`, `u16`, `u32`, `i32`, `i64` | Fixed width, little-endian, two's complement. No variable-length integers. |
| `f32`, `f64` | IEEE 754, little-endian. NaN and infinities are written as they are. |
| `bool` | `u8`, `0` or `1`. A reader rejects any other value. |
| `string` | `u32` byte length, then that many bytes of UTF-8. No terminator. |
| `list<T>` | `u32` element count, then the elements in order. |
| `option<T>` | `u8` tag, `0` for none and `1` for some, then `T` if some. A reader rejects other tags. |
| `map<K, V>` | `u32` entry count, then `K` and `V` alternating, in the key order of the writer's `BTreeMap`. |
| enum | `u8` discriminator, then the fields of that variant in the order listed. Discriminators are assigned explicitly, never reordered and never reused. A reader rejects an unknown discriminator. |
| struct | Its fields in the order listed, concatenated. |
| `extern` | `u32` index into the matching `Externals` table. |
| `param` | `string`: the animator parameter name. |
| `vec2`, `vec3`, `vec4`, `color` | `f64` × 2, 3, 4, 4. Color is r, g, b, a. |
| `quat` | `f64` × 4 in x, y, z, w order. |

Indices into state lists and similar are `u32`. Counts never exceed `u32`.

A reader accepts only complete, valid encodings. Every read must fit within the remaining payload, and lengths, counts and indices must be representable in the reader's native size type without truncation or overflow. Declared lengths and counts must match the bytes and elements consumed. After reading the root value, no payload bytes may remain. A writer rejects lengths, counts, indices or numeric conversions that cannot be represented in their wire types instead of truncating or wrapping them.

Strings must be valid UTF-8; invalid sequences are rejected rather than replaced. Every discriminator must be one of the values listed for its field, including fields written as `u8` with named alternatives. Map keys and animation targets within a `ValueSet` must be unique; a reader rejects duplicates instead of silently overwriting them. Every external and state index must be in range for its corresponding table or layer. Validation that requires a later field, such as a layer's `default_state`, completes before the enclosing value is accepted. Decoding fails as a whole on any violation and returns no partial value.

## Avatar Payload

This section is what `data_version` of the `DA2a` blob covers. Any change here bumps it.

The order differs from the Rust struct: `Externals` comes first so that a client has every reference table before it reads the body that indexes into them. Assets and component types can be resolved at that point; object path resolution also depends on the referencing controller's `path_mode`.

```
Avatar:
  externals             : Externals
  expression_parameters : list<ExpressionParameter>
  controllers           : list<PlayableController>
  menu                  : list<MenuItem>
```

### Externals

```
Externals:
  object_paths        : list<ExternEntry<string>>
  component_types     : list<ExternEntry<string>>
  assets              : list<ExternEntry<AssetLocator>>
  needs_relative_root : bool

ExternEntry<V>:
  value         : V
  referenced_at : list<SourceLocation>

SourceLocation:
  chunk : string
  line  : u32

AssetLocator: enum
  0 Guid   : guid string
  1 Path   : path string
  2 Named  : asset_type string, name string
```

An `extern` anywhere in the body indexes the table of its kind: object paths, component types or assets. Indices are always in range for a blob the core produced; a reader still bounds-checks them.

The object path table interns path strings independently of `path_mode`. The same index can therefore refer to different objects in absolute and relative controllers. The client resolves and caches objects by `(path_mode, index)`, against the avatar root for Absolute and the supplied root for Relative. It validates only combinations actually referenced by the body; a path used only in a relative controller need not exist under the avatar root. `needs_relative_root` requests the supplied root and does not require every path to resolve under both roots.

### Expression Parameters

```
ExpressionParameter:
  name   : string
  kind   : enum
    0 Bool  : default option<bool>
    1 Int   : width u8, default option<i32>
    2 Float : width u8, default option<f32>
  saved  : bool
  synced : bool
```

`width` is the bit width; `0` means the script did not specify one.

### Controllers

```
PlayableController:
  playable   : u8   0 Base, 1 Additive, 2 Gesture, 3 Action, 4 Fx, 5 Sitting, 6 TPose, 7 IkPose
  mode       : u8   0 Append, 1 Replace
  priority   : i32
  path_mode  : u8   0 Absolute, 1 Relative
  mask       : option<extern>              (asset)
  parameters : list<AnimatorParameter>
  layers     : list<AnimatorLayer>

AnimatorParameter:
  name : string
  kind : enum
    0 Bool  : default option<bool>
    1 Int   : default option<i32>
    2 Float : default option<f32>

AnimatorLayer:
  name          : string
  default_state : option<u32>              (index into states)
  states        : list<AnimatorState>
  transitions   : list<AnimatorTransition>

AnimatorState:
  name           : string
  motion         : option<Motion>
  speed          : f64
  speed_by       : option<param>
  time_by        : option<param>
  write_defaults : bool
  behaviors      : list<Behavior>

AnimatorTransition:
  from       : enum
    0 Entry
    1 State  : index u32
  to         : enum
    0 State  : index u32
    1 Exit
  duration   : f64
  conditions : list<Condition>

Condition: enum
  0 If        : param
  1 IfNot     : param
  2 Equals    : param, value i64
  3 NotEqual  : param, value i64
  4 Greater   : param, value f64
  5 Less      : param, value f64
```

### Motions

The Rust model nests `Motion::Clip` / `Motion::BlendTree` and then `Clip::Inline` / `Clip::External` and `BlendTree::Parametric` / `BlendTree::Direct`. The wire flattens those into one discriminator.

```
Motion: enum
  0 InlineClip     : animation InlineAnimation
  1 ExternalClip   : asset extern
  2 ParametricTree : tree_type u8, x param, y option<param>, fields list<ParametricField>
  3 DirectTree     : fields list<DirectField>

tree_type: 0 Linear, 1 Simple2d, 2 Freeform2d, 3 Cartesian2d

ParametricField:
  position : f64 × 2                       (a linear tree uses the first component only)
  speed    : f64
  motion   : Motion

DirectField:
  weight_by : param
  speed     : f64
  motion    : Motion
```

### Animations

```
InlineAnimation: enum
  0 Fixed : entries list<FixedEntry>
  1 Keyed : attributes ClipAttributes, curves list<KeyedEntry>

FixedEntry:
  target : AnimatedTarget
  value  : AnimatedValue

KeyedEntry:
  target : AnimatedTarget
  curve  : Curve

ClipAttributes:
  length       : f64
  loop_time    : bool
  loop_blend   : bool
  cycle_offset : f64

Curve:
  first : Keyframe
  rest  : list<Segment>

Segment:
  interpolation : enum
    0 Constant
    1 Linear
    2 Bezier : x1 f64, y1 f64, x2 f64, y2 f64
  keyframe      : Keyframe

Keyframe:
  time  : f64
  value : AnimatedValue
```

Entries of a fixed or keyed animation are written in the key order of the writer's `ValueSet`. A reader must not depend on that order.

### Animated Targets and Values

```
AnimatedTarget: enum
  0 AnimatorSelf : property enum
      0 ParameterFloatValue : name param
  1 GameObject   : path extern, property u8
      0 Active, 1 TransformPosition, 2 TransformRotationQuaternion, 3 TransformRotationEuler, 4 TransformScale
  2 Renderer     : path extern, renderer_type string, property enum
      0 Enabled
      1 BlendShape       : name string
      2 Material         : slot u32
      3 MaterialProperty : name string
      4 Serialized       : name string
  3 Component    : path extern, component_type extern, property enum, value_type u8
      0 Enabled
      1 Serialized       : name string

value_type and AnimatedValue discriminators share one numbering:
  0 Float, 1 Int, 2 Bool, 3 Vector2, 4 Vector3, 5 Vector4, 6 Quaternion, 7 Color, 8 ObjectReference

AnimatedValue: enum
  0 Float           : f64
  1 Int             : i64
  2 Bool            : bool
  3 Vector2         : vec2
  4 Vector3         : vec3
  5 Vector4         : vec4
  6 Quaternion      : quat
  7 Color           : color
  8 ObjectReference : asset extern
```

Where the model holds `AnimatedValue<()>` (menu controls and parameter drives), discriminator `8` never appears. A reader rejects it there.

### State Behaviors

```
Behavior: enum
  0 ParameterDrive  : target ParameterDriveTarget
  1 TrackingControl : modes u8 × 10
  2 Generic         : type_name string, fields map<string, GenericValue>

ParameterDriveTarget: enum
  0 Set         : parameter param, value AnimatedValue
  1 Add         : parameter param, value AnimatedValue
  2 RandomInt   : parameter param, min i64, max i64
  3 RandomBool  : parameter param, chance f64
  4 RandomFloat : parameter param, min f64, max f64
  5 Copy        : from param, to param
  6 RangedCopy  : from param, from_min f64, from_max f64, to param, to_min f64, to_max f64

TrackingControl modes, one byte each, in this fixed order:
  Head, LeftHand, RightHand, Hip, LeftFoot, RightFoot, LeftFingers, RightFingers, Eyes, Mouth
  0 NoChange, 1 Tracking, 2 Animation
  A target the model does not mention is written as NoChange.

GenericValue: enum
  0 Bool   : bool
  1 Int    : i64
  2 Float  : f64
  3 String : string
  4 List   : list<GenericValue>
  5 Map    : map<string, GenericValue>
```

### Menu

```
MenuItem: enum
  0 SubMenu  : name string, items list<MenuItem>
  1 Toggle   : name string, parameter param, value AnimatedValue
  2 Button   : name string, parameter param, value AnimatedValue
  3 Radial   : name string, axis MenuAxis
  4 TwoAxis  : name string, horizontal MenuAxis, vertical MenuAxis
  5 FourAxis : name string, up MenuAxis, down MenuAxis, left MenuAxis, right MenuAxis

MenuAxis:
  parameter : param
  positive  : option<string>
  negative  : option<string>
```

## Diagnostics Payload

This section is what `data_version` of the `DA2d` blob covers. It has its own counter, independent of the Avatar one.

```
Diagnostics:
  stage : u8   0 Script, 1 Transform
  items : list<Diagnostic>

Diagnostic:
  at      : option<SourceLocation>
  message : string
```

A `Script` stage blob carries exactly one item whose message includes the Lua traceback when available and whose location is none. A `Transform` stage blob carries one item per `TransformError`, with the location the transform attached, if any.

## Versioning

Both versions are matched exactly. A reader accepts a blob only when `schema_version` and the `data_version` for that magic equal its own constants. The native library and the client ship together and a blob is never persisted, so there is no consumer of an older layout to stay compatible with. The versions exist to turn a mismatch into a clear error, not to enable compatibility.

Should compiled output ever be cached, the cache key includes both versions; a mismatch is a cache miss.

Bump `schema_version` when any rule in the Encoding Rules section changes: the width of a length, the width of a discriminator, the representation of `option`, and so on.

Bump the Avatar `data_version` when the Avatar payload changes in any way: a field added, removed or reordered, an enum variant added, a discriminator renumbered. Bump the Diagnostics `data_version` under the same conditions for its payload. Adding a variant is a breaking change too, because an older reader rejects the unknown discriminator.

Each version is one constant on the Rust side and one on the client side, changed together in one commit. Format tests construct model values directly in Rust, without Lua evaluation or transformation. Fixtures cover both Avatar and Diagnostics, every wire variant (including ones not exposed to Lua), optional fields and boundary values. Tests check encoding against checked-in golden bytes and decoding against the expected values. Rust and C# readers use the same golden blobs.

For each encoded Rust enum, `enum_cases!` in `declavatar2/src/test_support.rs` takes one entry per variant: an explicit variant pattern paired with a fixture expression. From this single list it generates both an exhaustive `match` without a wildcard fallback and a test per fixture, each calling the shared verification body. Each case checks that its fixture matches its paired pattern; format tests supply encoding and decoding checks in the verification body. Adding a variant without a corresponding fixture then fails to compile. The macro rejects catch-all and top-level alternative patterns and rejects unreachable duplicate patterns. Nested enums are covered separately, including enums flattened into a single wire discriminator. This checks variant coverage, not every possible field value; boundary and malformed-input cases remain separate tests. Unit tests import the macro through `crate::test_support`; integration tests load the same file with `#[path = "../src/test_support.rs"] mod support` and use `support::enum_cases`.

Golden comparisons detect changes exercised by the fixtures, but regenerating golden files alone does not prove that the matching version was bumped. Every layout change still requires the appropriate version update in the same change; code review checks this alongside the fixture and golden changes.

## Implementation Notes

- The encoder and decoder live in a `declavatar2::interop` module: a `Writer` over `Vec<u8>` and a `Reader` over `&[u8]` for the primitives and the header, and `Encode` / `Decode` implemented by hand for every type in this document. Discriminators are written as literal constants next to the type they belong to, so the Rust and C# implementations can be read side by side.
- `AnimatorParameterTypeDefault` retains typed defaults (`bool`, `i32`, `f32`) in the model and transform. Integer defaults outside the `i32` range are transform errors, including for internal parameters.
- `serde` and `rmp-serde` are not used by this format and are removed together with the partial `Serialize` implementations they supported. The `serialize` method of the `StateBehavior` trait is subsumed by the `Behavior` encoding.
- The FFI returns one blob as a pointer and a length, owned by the native side and released through a matching free function. The magic makes the blob self-identifying, so one compile entry point returns either kind.
- Strings are not interned. Parameter names repeat often, but a blob is tens of kilobytes at most. A string table can be added under a `schema_version` bump if that ever matters.
- Floats stay `f64` on the wire wherever the model holds `f64`; converting to `float` for Unity is the client's job.
