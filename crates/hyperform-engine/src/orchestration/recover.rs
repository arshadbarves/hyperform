//! On session open, git forgets worktrees whose directories are gone
//! (removed by hand or by a crash mid-removal). Nothing on disk is
//! deleted: pending worktrees of finished agents stay for the user to apply
//! or discard, and a work-plan task's worktree (pending or kept after a
//! conflict) stays until its change is applied, discarded or retried.

use hyperform_host::{is_repo, prune_worktrees};

use crate::session::SessionCore;

pub(crate) async fn prune_stale_worktrees(core: &SessionCore) {
    if !is_repo(&core.root).await {
        return;
    }
    if let Err(error) = prune_worktrees(&core.root).await {
        tracing::warn!(%error, "could not prune stale agent worktrees");
    }
}
