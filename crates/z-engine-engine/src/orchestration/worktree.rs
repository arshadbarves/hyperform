//! Worktree isolation: an agent works in `.hyperform/worktrees/<agent>` on
//! the branch `hyperform/<agent>` from HEAD. At the end its changes are
//! committed and summarized; applying merges `base..branch` into the
//! project tree (a conflict leaves the tree untouched and keeps the
//! worktree), discarding removes the worktree and its branch.

use std::path::{Path, PathBuf};

use z_engine_context::render_template;
use z_engine_host::{
    apply_patch, commit_all, create_worktree, diff_range, diffstat_range, git, is_repo,
    remove_worktree,
};
use z_engine_prompts::system::WORKTREE;
use z_engine_protocol::{AgentId, AgentInfo, WorktreeInfo, WorktreeState};

use super::blueprint::Placement;
use super::tracker::publish;
use crate::run::WorktreeScope;
use crate::session::{SessionCore, git_info};

/// A fresh worktree for `agent`; `Ok(None)` outside a git repository.
pub(crate) async fn prepare(
    core: &SessionCore,
    agent: &AgentId,
) -> Result<Option<(Placement, WorktreeInfo)>, String> {
    if !is_repo(&core.root).await {
        return Ok(None);
    }
    let path = core
        .root
        .join(".hyperform")
        .join("worktrees")
        .join(agent.as_str());
    let branch = format!("hyperform/{agent}");
    let handle = create_worktree(&core.root, &path, &branch, None)
        .await
        .map_err(|error| error.to_string())?;
    let root = tokio::fs::canonicalize(&handle.path)
        .await
        .map_err(|error| error.to_string())?;
    let info = WorktreeInfo {
        path: root.to_string_lossy().into_owned(),
        branch: handle.branch.clone(),
        base: handle.base_sha.clone(),
        state: WorktreeState::Pending,
        files_changed: 0,
        diffstat: String::new(),
    };
    Ok(Some((placement(core, root, &info).await, info)))
}

/// Re-enters an existing worktree (a resumed agent).
pub(crate) async fn reenter(core: &SessionCore, info: &WorktreeInfo) -> Option<Placement> {
    let usable = matches!(
        info.state,
        WorktreeState::Pending | WorktreeState::Conflicted
    );
    let root = PathBuf::from(&info.path);
    (usable && root.is_dir()).then_some(())?;
    Some(placement(core, root, info).await)
}

async fn placement(core: &SessionCore, root: PathBuf, info: &WorktreeInfo) -> Placement {
    let project = core.root.to_string_lossy();
    let note = render_template(
        WORKTREE,
        &[
            ("path", &info.path),
            ("branch", &info.branch),
            ("base", short(&info.base)),
            ("project", &project),
        ],
    )
    .trim()
    .to_string();
    Placement {
        worktree: Some(WorktreeScope {
            project: core.root.clone(),
            git: git_info(&root).await,
            note,
        }),
        root,
    }
}

/// Commits the agent's changes and summarizes them. A worktree without
/// changes is removed at once (`Empty`).
pub(crate) async fn finalize(
    core: &SessionCore,
    agent: &AgentId,
    description: &str,
    mut info: WorktreeInfo,
) -> Result<WorktreeInfo, String> {
    let path = PathBuf::from(&info.path);
    let message = format!("HyperForm agent {agent}: {description}");
    commit_all(&path, &message)
        .await
        .map_err(|error| error.to_string())?;
    let (files, stat) = diffstat_range(&core.root, &info.base, &info.branch)
        .await
        .map_err(|error| error.to_string())?;
    info.files_changed = files;
    info.diffstat = stat.trim_end().to_string();
    info.state = if files == 0 {
        remove(core, &path, &info.branch).await;
        WorktreeState::Empty
    } else {
        WorktreeState::Pending
    };
    Ok(info)
}

/// Root-relative files `base..branch` changes; `None` when git cannot tell.
pub(crate) async fn changed_files(core: &SessionCore, info: &WorktreeInfo) -> Option<Vec<String>> {
    if info.state == WorktreeState::Empty {
        return Some(Vec::new());
    }
    let args = ["diff", "--name-only", &info.base, &info.branch, "--"];
    match git(&core.root, &args).await {
        Ok(names) => Some(
            names
                .lines()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(String::from)
                .collect(),
        ),
        Err(error) => {
            tracing::debug!(%error, branch = %info.branch, "no file list for a worktree diff");
            None
        }
    }
}

/// What applying a worktree agent's changes did.
#[derive(Debug)]
pub(crate) struct Applied {
    /// Changes reached the project tree.
    pub merged: bool,
    pub summary: String,
    /// The files the change touched.
    pub files: Vec<String>,
    /// Where it conflicts (empty when merged).
    pub conflicts: Vec<String>,
    /// The worktree, kept after a conflict.
    pub worktree: String,
}

pub(crate) async fn apply(core: &SessionCore, agent: &AgentId) -> Result<Applied, String> {
    let _merging = core.agents.merge_lock().await;
    let (mut info, mut worktree) = decidable(core, agent)?;
    let files = changed_files(core, &worktree).await.unwrap_or_default();
    let patch = diff_range(&core.root, &worktree.base, &worktree.branch)
        .await
        .map_err(|error| error.to_string())?;
    let outcome = apply_patch(&core.root, &patch)
        .await
        .map_err(|error| error.to_string())?;
    let summary = if outcome.applied {
        remove(core, Path::new(&worktree.path), &worktree.branch).await;
        worktree.state = WorktreeState::Applied;
        format!(
            "Applied {} changed file(s) from agent {agent} to the project tree.\n{}",
            worktree.files_changed, worktree.diffstat
        )
    } else {
        worktree.state = WorktreeState::Conflicted;
        format!(
            "The changes of agent {agent} conflict with the project tree in: {}. Nothing was \
             changed; the worktree is kept at {}.\n{}",
            outcome.conflicts.join(", "),
            worktree.path,
            outcome.message
        )
    };
    let path = worktree.path.clone();
    info.worktree = Some(worktree);
    publish(core, &info, false);
    Ok(Applied {
        merged: outcome.applied,
        summary,
        files,
        conflicts: outcome.conflicts,
        worktree: path,
    })
}

pub(crate) async fn discard(core: &SessionCore, agent: &AgentId) -> Result<AgentInfo, String> {
    let _merging = core.agents.merge_lock().await;
    let (mut info, mut worktree) = decidable(core, agent)?;
    remove(core, Path::new(&worktree.path), &worktree.branch).await;
    worktree.state = WorktreeState::Discarded;
    info.worktree = Some(worktree);
    publish(core, &info, false);
    Ok(info)
}

/// Removes the fresh worktree of an agent that never ran.
pub(crate) async fn abandon(core: &SessionCore, info: &WorktreeInfo) {
    remove(core, Path::new(&info.path), &info.branch).await;
}

/// A finished agent whose worktree awaits a decision.
fn decidable(core: &SessionCore, agent: &AgentId) -> Result<(AgentInfo, WorktreeInfo), String> {
    let info = core
        .with_state(|state| {
            let found = state.agents.iter().find(|info| info.agent_id == *agent);
            found.cloned()
        })
        .ok_or_else(|| format!("no agent {agent} in this session"))?;
    if !info.status.is_terminal() {
        return Err(format!("agent {agent} is still running"));
    }
    let worktree = info.worktree.clone().ok_or_else(|| {
        format!("agent {agent} worked in the shared tree; there is nothing to apply")
    })?;
    match worktree.state {
        WorktreeState::Pending | WorktreeState::Conflicted => Ok((info, worktree)),
        state => Err(format!(
            "agent {agent}'s worktree is {} and has nothing to apply",
            label(state)
        )),
    }
}

/// Removes the worktree and deletes its branch; a worktree that is already
/// gone still loses its branch.
async fn remove(core: &SessionCore, path: &Path, branch: &str) {
    if let Err(error) = remove_worktree(&core.root, path, true).await {
        tracing::debug!(%error, path = %path.display(), "worktree removal failed; deleting the branch");
        if let Err(error) = git(&core.root, &["worktree", "prune"]).await {
            tracing::debug!(%error, "could not prune worktrees");
        }
        if let Err(error) = git(&core.root, &["branch", "-D", branch]).await {
            tracing::warn!(%error, branch, "could not delete an agent branch");
        }
    }
}

fn label(state: WorktreeState) -> &'static str {
    match state {
        WorktreeState::Pending => "pending",
        WorktreeState::Applied => "already applied",
        WorktreeState::Discarded => "discarded",
        WorktreeState::Conflicted => "conflicted",
        WorktreeState::Empty => "empty",
    }
}

fn short(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}
