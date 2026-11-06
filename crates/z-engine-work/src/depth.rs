//! Planning-depth signals: how much structure a plan deserves, from its
//! task count, how widely its scopes spread, and whether it touches paths
//! the user marked sensitive (`[work] sensitive_paths`).

use std::collections::BTreeSet;

use z_engine_protocol::work::{PlanDepth, TaskScope};

use crate::graph::is_closed;
use crate::scope::{base_dir, overlap};
use crate::state::PlanState;

/// At or above this many tasks a plan is Deep.
pub const DEEP_TASKS: u32 = 8;
/// At or above this many scope directories a plan is Deep.
pub const DEEP_DIRS: u32 = 5;
/// At or below this many tasks (in at most one directory) a plan is Light.
pub const LIGHT_TASKS: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepthSignals {
    pub tasks: u32,
    /// Distinct scope patterns.
    pub scope_files: u32,
    /// Distinct directories the scopes live in.
    pub scope_dirs: u32,
    /// Scope patterns that overlap a sensitive path.
    pub sensitive: Vec<String>,
    pub suggested: PlanDepth,
}

/// Signals for tasks with these scopes (one scope per task).
pub fn assess<'a>(
    scopes: impl IntoIterator<Item = &'a TaskScope>,
    sensitive_paths: &[String],
) -> DepthSignals {
    let mut tasks = 0u32;
    let mut files = BTreeSet::new();
    for scope in scopes {
        tasks += 1;
        files.extend(scope.files.iter().cloned());
    }
    let dirs: BTreeSet<&str> = files.iter().map(|file| base_dir(file)).collect();
    let scope_dirs = count(dirs.len());
    let files: Vec<String> = files.into_iter().collect();
    let sensitive = overlap(&files, sensitive_paths);
    let suggested = if tasks >= DEEP_TASKS || scope_dirs >= DEEP_DIRS || !sensitive.is_empty() {
        PlanDepth::Deep
    } else if tasks <= LIGHT_TASKS && scope_dirs <= 1 {
        PlanDepth::Light
    } else {
        PlanDepth::Standard
    };
    DepthSignals {
        tasks,
        scope_files: count(files.len()),
        scope_dirs,
        sensitive,
        suggested,
    }
}

/// Signals for a plan's tasks that are not closed.
pub fn assess_plan(state: &PlanState, sensitive_paths: &[String]) -> DepthSignals {
    let scopes = state
        .tasks
        .values()
        .filter(|task| !is_closed(&task.status))
        .map(|task| &task.scope);
    assess(scopes, sensitive_paths)
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(files: &[&str]) -> TaskScope {
        TaskScope {
            files: files.iter().map(|file| file.to_string()).collect(),
            symbols: Vec::new(),
        }
    }

    #[test]
    fn small_local_work_is_light() {
        let scopes = [scope(&["src/parse.rs"]), scope(&["src/lex.rs"])];
        let signals = assess(&scopes, &[]);
        assert_eq!(
            (signals.tasks, signals.scope_files, signals.scope_dirs),
            (2, 2, 1)
        );
        assert_eq!(signals.suggested, PlanDepth::Light);
    }

    #[test]
    fn spread_or_many_tasks_get_more_depth() {
        let three = [scope(&["a/x.rs"]), scope(&["b/y.rs"]), scope(&[])];
        assert_eq!(assess(&three, &[]).suggested, PlanDepth::Standard);
        let many: Vec<TaskScope> = (0..DEEP_TASKS).map(|_| scope(&[])).collect();
        assert_eq!(assess(&many, &[]).suggested, PlanDepth::Deep);
        let wide = [scope(&["a/1", "b/1", "c/1", "d/1", "e/1"])];
        assert_eq!(assess(&wide, &[]).scope_dirs, 5);
        assert_eq!(assess(&wide, &[]).suggested, PlanDepth::Deep);
    }

    #[test]
    fn sensitive_paths_make_a_plan_deep() {
        let scopes = [scope(&["src/auth/*.rs"])];
        let signals = assess(&scopes, &["src/auth/**".to_string()]);
        assert_eq!(signals.sensitive, vec!["src/auth/*.rs".to_string()]);
        assert_eq!(signals.suggested, PlanDepth::Deep);
    }
}
