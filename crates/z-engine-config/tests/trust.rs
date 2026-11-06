//! Workspace trust is stored per canonical project root.

#[allow(dead_code)]
mod support;

use support::{fixture, write};
use z_engine_config::{ConfigError, TrustStore};

#[test]
fn trust_round_trips_through_the_file() {
    let f = fixture();
    let mut store = TrustStore::load(&f.paths.trust_file).unwrap();
    assert!(!store.is_trusted(&f.project));
    assert!(store.trust(&f.project));
    assert!(
        !store.trust(&f.project.join(".git/..")),
        "same root after canonicalization"
    );
    store.save(&f.paths.trust_file).unwrap();

    let mut loaded = TrustStore::load(&f.paths.trust_file).unwrap();
    assert_eq!(loaded, store);
    assert!(loaded.is_trusted(&f.project));
    assert!(
        !loaded.is_trusted(&f.project.join(".git")),
        "trust is not inherited"
    );
    assert_eq!(loaded.roots().count(), 1);

    assert!(loaded.revoke(&f.project));
    assert!(!loaded.revoke(&f.project));
    loaded.save(&f.paths.trust_file).unwrap();
    assert!(
        !TrustStore::load(&f.paths.trust_file)
            .unwrap()
            .is_trusted(&f.project)
    );
}

#[cfg(unix)]
#[test]
fn symlinked_roots_resolve_to_the_same_entry() {
    let f = fixture();
    let link = f.tmp.path().join("link");
    std::os::unix::fs::symlink(&f.project, &link).unwrap();
    let mut store = TrustStore::default();
    store.trust(&link);
    assert!(store.is_trusted(&f.project));
}

#[test]
fn a_deleted_root_can_still_be_revoked() {
    let f = fixture();
    let gone = f.tmp.path().join("gone");
    std::fs::create_dir_all(&gone).unwrap();
    let mut store = TrustStore::default();
    store.trust(&gone);
    let stored = store.roots().next().unwrap().to_string();
    std::fs::remove_dir(&gone).unwrap();
    assert!(store.revoke(std::path::Path::new(&stored)));
}

#[test]
fn a_bare_list_of_roots_is_accepted() {
    let f = fixture();
    let root = std::fs::canonicalize(&f.project).unwrap();
    let list = serde_json::to_string(&[root.to_string_lossy()]).unwrap();
    write(&f.paths.trust_file, &list);
    assert!(
        TrustStore::load(&f.paths.trust_file)
            .unwrap()
            .is_trusted(&f.project)
    );
}

#[test]
fn malformed_file_is_an_error() {
    let f = fixture();
    write(&f.paths.trust_file, "[1, 2");
    assert!(matches!(
        TrustStore::load(&f.paths.trust_file),
        Err(ConfigError::Parse { .. })
    ));
}
