//! Artifacts: unique, sanitized file names inside `<session>/artifacts/`.

mod support;

use std::fs;

use support::{SESSION, new_session, temp_store};
use z_engine_protocol::SessionId;

#[test]
fn text_artifacts_get_unique_names_inside_the_session() {
    let (_dir, store) = temp_store();
    drop(store.create(new_session(SESSION)).unwrap());
    let artifacts = store.artifacts(&SessionId::from(SESSION));
    assert_eq!(artifacts.dir(), store.dir().join(SESSION).join("artifacts"));

    let first = artifacts
        .write_text("Bash output", "log", "line 1\nline 2\n")
        .unwrap();
    let second = artifacts.write_text("Bash output", "log", "other").unwrap();
    assert_ne!(first, second);
    for path in [&first, &second] {
        assert_eq!(path.parent(), Some(artifacts.dir()));
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("bash-output-"), "{name}");
        assert!(name.ends_with(".log"), "{name}");
        // "bash-output-" + 26-char ULID + ".log"
        assert_eq!(name.len(), "bash-output-".len() + 26 + ".log".len());
    }
    assert_eq!(fs::read_to_string(&first).unwrap(), "line 1\nline 2\n");
}

#[test]
fn byte_artifacts_round_trip_with_default_extensions() {
    let (_dir, store) = temp_store();
    drop(store.create(new_session(SESSION)).unwrap());
    let artifacts = store.artifacts(&SessionId::from(SESSION));
    let png = [0x89, b'P', b'N', b'G', 0, 1, 2];
    let image = artifacts.write_bytes("screenshot", "", &png).unwrap();
    assert!(image.to_str().unwrap().ends_with(".bin"));
    assert_eq!(fs::read(&image).unwrap(), png);
    let text = artifacts.write_text("", "", "x").unwrap();
    let name = text.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("artifact-") && name.ends_with(".txt"),
        "{name}"
    );
}

#[test]
fn hints_and_extensions_cannot_escape_the_artifact_directory() {
    let (dir, store) = temp_store();
    drop(store.create(new_session(SESSION)).unwrap());
    let artifacts = store.artifacts(&SessionId::from(SESSION));
    let path = artifacts
        .write_text("../../../outside", "/../../x", "data")
        .unwrap();
    assert_eq!(path.parent(), Some(artifacts.dir()));
    assert!(!dir.path().join("outside").exists());
    let entries = fs::read_dir(artifacts.dir()).unwrap().count();
    assert_eq!(entries, 1);
}
