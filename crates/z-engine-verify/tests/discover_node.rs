mod support;

use std::path::PathBuf;

use support::{Fixture, check, command, has_note, ids, roots};
use z_engine_protocol::CheckKind;

const SCRIPTS: &str = r#"{"scripts":{"test":"jest","lint":"eslint .","typecheck":"tsc --noEmit","build":"vite build","dev":"vite"}}"#;

#[test]
fn lockfiles_choose_the_package_manager() {
    for (lock, test, lint) in [
        ("pnpm-lock.yaml", "pnpm test", "pnpm run lint"),
        ("yarn.lock", "yarn test", "yarn run lint"),
        ("bun.lockb", "bun run test", "bun run lint"),
        ("bun.lock", "bun run test", "bun run lint"),
        ("package-lock.json", "npm test", "npm run lint"),
    ] {
        let fixture = Fixture::new(&[("package.json", SCRIPTS), (lock, "")]);
        let profile = fixture.discover();
        let manager = test.split(' ').next().unwrap();
        assert_eq!(
            command(&profile, &format!("{manager}:test")),
            test,
            "{lock}"
        );
        assert_eq!(
            command(&profile, &format!("{manager}:lint")),
            lint,
            "{lock}"
        );
    }
    let profile = Fixture::new(&[("package.json", SCRIPTS)]).discover();
    assert_eq!(
        ids(&profile),
        ["npm:test", "npm:build", "npm:typecheck", "npm:lint"]
    );
    assert_eq!(command(&profile, "npm:typecheck"), "npm run typecheck");
    assert_eq!(check(&profile, "npm:build").kind, CheckKind::Build);
    assert_eq!(check(&profile, "npm:typecheck").kind, CheckKind::Typecheck);
}

#[test]
fn package_manager_field_applies_without_a_lockfile() {
    let fixture = Fixture::new(&[(
        "package.json",
        r#"{"packageManager":"pnpm@9.12.0","scripts":{"check":"svelte-check"}}"#,
    )]);
    let profile = fixture.discover();
    assert_eq!(command(&profile, "pnpm:check"), "pnpm run check");
    assert_eq!(check(&profile, "pnpm:check").kind, CheckKind::Typecheck);
}

#[test]
fn monorepo_packages_are_roots_with_the_workspace_manager() {
    let fixture = Fixture::new(&[
        (
            "package.json",
            r#"{"private":true,"workspaces":["packages/*","apps/*"],"scripts":{"test":"turbo run test"}}"#,
        ),
        ("pnpm-lock.yaml", ""),
        (
            "pnpm-workspace.yaml",
            "packages:\n  - packages/*\n  - apps/*\n",
        ),
        (
            "packages/ui/package.json",
            r#"{"scripts":{"test":"vitest","lint":"eslint src"}}"#,
        ),
        ("packages/utils/package.json", r#"{"name":"utils"}"#),
        (
            "apps/web/package.json",
            r#"{"scripts":{"build":"vite build","type-check":"vue-tsc --noEmit"}}"#,
        ),
        (
            "apps/web/node_modules/dep/package.json",
            r#"{"scripts":{"test":"x"}}"#,
        ),
    ]);
    let profile = fixture.discover();
    assert_eq!(
        roots(&profile),
        [
            (".", "node"),
            ("apps/web", "node"),
            ("packages/ui", "node"),
            ("packages/utils", "node"),
        ]
    );
    assert_eq!(
        ids(&profile),
        [
            "pnpm:test",
            "apps/web/pnpm:build",
            "apps/web/pnpm:type-check",
            "packages/ui/pnpm:test",
            "packages/ui/pnpm:lint",
        ]
    );
    let ui = check(&profile, "packages/ui/pnpm:test");
    assert_eq!(ui.command, "pnpm exec vitest run");
    assert_eq!(ui.cwd, PathBuf::from("packages/ui"));
    assert_eq!(ui.label, "pnpm exec vitest run (packages/ui)");
    assert_eq!(
        command(&profile, "apps/web/pnpm:type-check"),
        "pnpm run type-check"
    );
    assert!(has_note(&profile, "packages/utils/package.json: no test"));
}

#[test]
fn a_nearer_lockfile_wins_and_mixed_lockfiles_are_noted() {
    let fixture = Fixture::new(&[
        ("package.json", r#"{"scripts":{"lint":"eslint ."}}"#),
        ("yarn.lock", ""),
        ("package-lock.json", ""),
        (
            "tools/bench/package.json",
            r#"{"scripts":{"test":"mocha"}}"#,
        ),
        ("tools/bench/bun.lock", ""),
    ]);
    let profile = fixture.discover();
    assert_eq!(command(&profile, "yarn:lint"), "yarn run lint");
    assert_eq!(command(&profile, "tools/bench/bun:test"), "bun run test");
    assert!(has_note(
        &profile,
        ".: several lockfiles; using yarn (yarn.lock)"
    ));
}

#[test]
fn watchers_placeholders_and_rewriting_scripts_are_skipped() {
    let fixture = Fixture::new(&[(
        "package.json",
        r#"{"scripts":{
            "test":"echo \"Error: no test specified\" && exit 1",
            "build":"tsc --watch",
            "lint":"eslint --fix .",
            "format":"prettier --write .",
            "format:check":"prettier --check .",
            "tsc":"tsc --noEmit",
            "check":123
        }}"#,
    )]);
    let profile = fixture.discover();
    assert_eq!(ids(&profile), ["npm:tsc", "npm:format:check"]);
    assert_eq!(
        command(&profile, "npm:format:check"),
        "npm run format:check"
    );
    assert_eq!(check(&profile, "npm:format:check").kind, CheckKind::Format);
    for (script, reason) in [
        ("test", "placeholder"),
        ("build", "watches"),
        ("lint", "rewrites"),
        ("format", "rewrites"),
    ] {
        assert!(
            has_note(&profile, &format!("script `{script}` is not a check: it")),
            "{script}"
        );
        assert!(
            profile
                .notes
                .iter()
                .any(|n| n.contains(script) && n.contains(reason))
        );
    }
}

#[test]
fn invalid_package_json_is_a_note() {
    let fixture = Fixture::new(&[
        ("package.json", "{\"scripts\": {"),
        ("web/package.json", "[1, 2]"),
    ]);
    let profile = fixture.discover();
    assert!(profile.roots.is_empty());
    assert!(has_note(&profile, "package.json: invalid JSON ("));
    assert!(has_note(&profile, "web/package.json: not a JSON object"));
}
