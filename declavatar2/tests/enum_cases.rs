#[path = "../src/test_support.rs"]
mod support;

use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

use rstest::rstest;
use support::enum_cases;

enum Example<T> {
    Unit,
    Tuple(T),
    Struct { value: T },
}

enum_cases! {
    fn all_shapes(value: Example<String>) {
        let actual = match value {
            Example::Unit => "unit".into(),
            Example::Tuple(value) | Example::Struct { value } => value,
        };
        assert!(["unit", "tuple", "struct"].contains(&actual.as_str()));
    }
    cases {
        unit: Example::Unit => Example::Unit,
        tuple: Example::Tuple(_) => Example::Tuple("tuple".into()),
        structure: Example::Struct { .. } => Example::Struct { value: "struct".into() },
    }
}

enum_cases! {
    fn mismatched_fixtures(value: Example<String>) {
        let _ = value;
        panic!("verification must not run for a mismatched fixture");
    }
    cases {
        #[should_panic(expected = "fixture does not match Example :: Unit")]
        unit: Example::Unit => Example::Tuple("wrong".into()),
        #[should_panic(expected = "fixture does not match Example :: Tuple")]
        tuple: Example::Tuple(_) => Example::Struct { value: "wrong".into() },
        #[should_panic(expected = "fixture does not match Example :: Struct")]
        structure: Example::Struct { .. } => Example::Unit,
    }
}

enum_cases! {
    fn verification_is_executed(value: Example<String>) {
        match value {
            Example::Unit => panic!("verified unit"),
            Example::Tuple(value) => panic!("verified {value}"),
            Example::Struct { value } => panic!("verified {value}"),
        }
    }
    cases {
        #[should_panic(expected = "verified unit")]
        unit: Example::Unit => Example::Unit,
        #[should_panic(expected = "verified tuple")]
        tuple: Example::Tuple(_) => Example::Tuple("tuple".into()),
        #[should_panic(expected = "verified struct")]
        structure: Example::Struct { .. } => Example::Struct { value: "struct".into() },
    }
}

struct CompileDirectory(PathBuf);

impl CompileDirectory {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "declavatar2-enum-cases-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for CompileDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[rstest]
#[case::complete("a: Choice::A => Choice::A, b: Choice::B => Choice::B", None)]
#[case::missing_variant("a: Choice::A => Choice::A", Some("E0004"))]
#[case::wildcard("all: _ => Choice::A", Some("no rules expected"))]
#[case::binding("all: value => Choice::A", Some("no rules expected"))]
#[case::alternatives("all: Choice::A | Choice::B => Choice::A", Some("no rules expected"))]
#[case::duplicate_variant(
    "a: Choice::A => Choice::A, also_a: Choice::A => Choice::A, b: Choice::B => Choice::B",
    Some("unreachable pattern")
)]
fn variant_coverage_is_checked_by_the_compiler(#[case] cases: &str, #[case] expected_error: Option<&str>) {
    let directory = CompileDirectory::new();
    let source = directory.0.join("cases.rs");
    let support = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test_support.rs");
    fs::write(&source, format!(
        "#[path = {support:?}] mod support;\nuse support::enum_cases;\nenum Choice {{ A, B }}\nenum_cases! {{\nfn cases(value: Choice) {{ let _ = value; }}\ncases {{ {cases} }}\n}}\n"
    )).unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2024", "--test", "--emit=metadata", "--crate-name", "enum_cases_check", "--out-dir"])
        .arg(&directory.0)
        .arg(&source)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    match expected_error {
        Some(expected) => {
            assert!(!output.status.success(), "invalid cases compiled successfully");
            assert!(stderr.contains(expected), "{stderr}");
        }
        None => assert!(output.status.success(), "{stderr}"),
    }
}
