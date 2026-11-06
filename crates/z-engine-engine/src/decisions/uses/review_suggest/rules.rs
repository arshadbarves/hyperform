//! The path rules: which changed files are in a risky area for sure, which
//! are worth asking about, and which shell commands deleted files.

/// A risky area, as the card names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Area {
    Authentication,
    Migrations,
    Ci,
    Deletions,
    /// An ambiguous file the decision model judged risky.
    Sensitive,
}

impl Area {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::Migrations => "database migrations",
            Self::Ci => "CI configuration",
            Self::Deletions => "deleted files",
            Self::Sensitive => "security-sensitive files",
        }
    }
}

const CI: &[&str] = &[
    ".github/workflows/",
    ".gitlab-ci.yml",
    ".circleci/",
    "jenkinsfile",
    "azure-pipelines.yml",
    ".buildkite/",
    "bitbucket-pipelines.yml",
    ".travis.yml",
];

const AUTH: &[&str] = &[
    "auth",
    "oauth",
    "login",
    "logout",
    "signin",
    "signup",
    "password",
    "passwd",
    "credential",
    "jwt",
    "sso",
    "saml",
    "rbac",
];

const AMBIGUOUS: &[&str] = &[
    "session",
    "token",
    "permission",
    "policy",
    "policies",
    "security",
    "secret",
    "crypto",
    "middleware",
    "guard",
    "cert",
    "tls",
    "deploy",
    "infra",
    "terraform",
    "dockerfile",
    "k8s",
    "helm",
    "env",
    "sql",
    "schema",
    "payment",
    "billing",
];

const DELETE_COMMANDS: &[&str] = &["rm", "rmdir", "unlink", "shred"];

/// The area a root-relative path is in for sure, if any.
pub(super) fn area_of(path: &str) -> Option<Area> {
    let lower = path.to_lowercase();
    if CI.iter().any(|marker| lower.contains(marker)) {
        return Some(Area::Ci);
    }
    let words = words(&lower);
    let migration = |word: &&str| word.starts_with("migration") || *word == "migrate";
    if words.iter().any(migration) || lower.contains("alembic/") {
        return Some(Area::Migrations);
    }
    let auth = |word: &&str| {
        let author = word.starts_with("author") && !word.starts_with("authoriz");
        !author && AUTH.iter().any(|prefix| word.starts_with(prefix))
    };
    words.iter().any(auth).then_some(Area::Authentication)
}

/// A path whose words hint at risk without settling it.
pub(super) fn ambiguous(path: &str) -> bool {
    let lower = path.to_lowercase();
    let words = words(&lower);
    words
        .iter()
        .any(|word| AMBIGUOUS.iter().any(|marker| word.starts_with(marker)))
}

/// Whether a `Bash: <command>` line of the turn deleted files; failed
/// commands (`(error)`) did not.
pub(super) fn deletes(line: &str) -> bool {
    let Some(command) = line.strip_prefix("Bash: ") else {
        return false;
    };
    if command.ends_with(" (error)") {
        return false;
    }
    let segments = command.split(['&', ';', '|', '\n']);
    segments.map(str::split_whitespace).any(|mut words| {
        let first = words.next().unwrap_or_default();
        let rest: Vec<&str> = words.collect();
        DELETE_COMMANDS.contains(&first)
            || (first == "git" && rest.first() == Some(&"rm"))
            || (first == "find" && rest.contains(&"-delete"))
    })
}

fn words(lower: &str) -> Vec<&str> {
    let split = lower.split(|ch: char| !ch.is_ascii_alphanumeric());
    split.filter(|word| !word.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_name_the_sure_areas() {
        for (path, area) in [
            (".github/workflows/ci.yml", Area::Ci),
            ("Jenkinsfile", Area::Ci),
            ("db/migrate/20240101_add_users.rb", Area::Migrations),
            ("crates/store/migrations/0003.sql", Area::Migrations),
            ("src/auth/session.ts", Area::Authentication),
            ("src/OAuthCallback.tsx", Area::Authentication),
            ("api/authorization.rs", Area::Authentication),
            ("web/login_form.svelte", Area::Authentication),
        ] {
            assert_eq!(area_of(path), Some(area), "{path}");
        }
        for path in [
            "src/authors.rs",
            "docs/author.md",
            "src/parser.rs",
            "README.md",
        ] {
            assert_eq!(area_of(path), None, "{path}");
        }
    }

    #[test]
    fn ambiguous_paths_are_asked_about() {
        assert!(ambiguous("src/session/store.rs"));
        assert!(ambiguous("deploy/Dockerfile"));
        assert!(ambiguous("config/.env.production"));
        assert!(ambiguous("src/env.ts"));
        assert!(!ambiguous("src/parser.rs"));
    }

    #[test]
    fn deletions_come_from_successful_shell_commands() {
        assert!(deletes("Bash: rm -rf build"));
        assert!(deletes("Bash: cargo clean && git rm src/old.rs"));
        assert!(deletes("Bash: find . -name '*.orig' -delete"));
        assert!(!deletes("Bash: rm -rf build (error)"));
        assert!(!deletes("Bash: cargo test"));
        assert!(!deletes("Edit: src/rm.rs"));
        assert!(!deletes("Bash: echo rm"));
    }
}
