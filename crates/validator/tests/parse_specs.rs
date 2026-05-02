//! Validates that every TOML spec in validator-specs/ parses successfully.

use std::path::Path;

#[test]
fn all_specs_parse() {
    let spec_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("validator-specs");

    let mut count = 0;
    for entry in std::fs::read_dir(&spec_dir).expect("read validator-specs dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            let raw = std::fs::read_to_string(&path).expect("read spec file");
            let _: validator::spec::Spec = toml::from_str(&raw)
                .unwrap_or_else(|e| panic!("parse error in {}: {e}", path.display()));
            count += 1;
        }
    }
    assert!(count > 0, "no .toml specs found in validator-specs/");
}
