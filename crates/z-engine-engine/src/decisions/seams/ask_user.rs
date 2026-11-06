//! Seam: the main agent called `AskUserQuestion` and the questions are
//! about to reach the user. The first note from an `on` use becomes the
//! tool result instead (the chat already answered); without one, the user
//! is asked exactly as today.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use z_engine_protocol::Question;

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::session::SessionCore;

pub(crate) async fn review_questions(
    core: &Arc<SessionCore>,
    cancel: &CancellationToken,
    questions: &[Question],
) -> Option<String> {
    review_questions_with(USES, core, cancel, questions).await
}

pub(super) async fn review_questions_with(
    uses: &[&'static dyn DecisionUse],
    core: &Arc<SessionCore>,
    cancel: &CancellationToken,
    questions: &[Question],
) -> Option<String> {
    let active = active(core, uses, Seam::AskUser);
    if active.is_empty() {
        return None;
    }
    let questions: Arc<[Question]> = Arc::from(questions);
    let notes = dispatch(core, active, cancel, move |decision_use, cx| {
        let questions = Arc::clone(&questions);
        Box::pin(async move { decision_use.ask_user(&cx, &questions).await })
    })
    .await;
    notes.into_iter().flatten().next()
}
