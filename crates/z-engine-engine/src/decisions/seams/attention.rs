//! Seam: something needs the user (an approval, a question, a plan to
//! review) or tells them something (a warning or error notice). The first
//! scores from an `on` use reach the GUI as `UrgencyScored`, where the
//! inbox sorts by them, and decide whether `Notification` hooks fire.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use z_engine_protocol::decisions::{Urgency, UrgencyInfo};
use z_engine_protocol::{ApprovalRequest, Event, NoticeLevel};

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::session::SessionCore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttentionKind {
    Approval,
    Question,
    Plan,
    Notice,
}

impl AttentionKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Approval => "approval",
            Self::Question => "question",
            Self::Plan => "plan",
            Self::Notice => "notice",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttentionItem {
    /// The request id, or `notice:<text>` for a notice: what the GUI
    /// matches the score to.
    pub key: String,
    pub kind: AttentionKind,
    pub text: String,
}

impl AttentionItem {
    pub(crate) fn approval(request: &ApprovalRequest) -> Self {
        let mut text = format!("{}: {}", request.tool, request.title);
        if !request.reason.is_empty() {
            text.push_str(&format!(" ({})", request.reason));
        }
        Self {
            key: request.request_id.as_str().to_string(),
            kind: AttentionKind::Approval,
            text,
        }
    }

    pub(crate) fn request(kind: AttentionKind, request_id: &str, text: String) -> Self {
        Self {
            key: request_id.to_string(),
            kind,
            text,
        }
    }

    pub(crate) fn notice(level: NoticeLevel, text: &str) -> Self {
        let level = if level == NoticeLevel::Error {
            "error"
        } else {
            "warning"
        };
        Self {
            key: format!("notice:{text}"),
            kind: AttentionKind::Notice,
            text: format!("{level}: {text}"),
        }
    }
}

/// Per item, the urgency an `on` use gave it (`None`: not scored).
pub(crate) async fn score_attention(
    core: &Arc<SessionCore>,
    items: Vec<AttentionItem>,
    cancel: &CancellationToken,
) -> Vec<Option<Urgency>> {
    score_attention_with(USES, core, items, cancel).await
}

pub(crate) async fn score_attention_with(
    uses: &[&'static dyn DecisionUse],
    core: &Arc<SessionCore>,
    items: Vec<AttentionItem>,
    cancel: &CancellationToken,
) -> Vec<Option<Urgency>> {
    let unscored = vec![None; items.len()];
    let active = active(core, uses, Seam::Attention);
    if active.is_empty() || items.is_empty() {
        return unscored;
    }
    let items: Arc<[AttentionItem]> = Arc::from(items);
    let asked = Arc::clone(&items);
    let scored = dispatch(core, active, cancel, move |decision_use, cx| {
        let items = Arc::clone(&asked);
        Box::pin(async move { decision_use.attention(&cx, &items).await })
    })
    .await;
    let Some(urgencies) = scored
        .into_iter()
        .find(|scores| scores.len() == items.len())
    else {
        return unscored;
    };
    for (item, urgency) in items.iter().zip(&urgencies) {
        if let Some(urgency) = *urgency {
            let key = item.key.clone();
            core.events.emit(Event::UrgencyScored {
                urgency: UrgencyInfo { key, urgency },
            });
        }
    }
    urgencies
}

/// Whether a `Notification` hook should fire: unless every item was
/// confidently scored below high.
pub(crate) fn worth_notifying(urgencies: &[Option<Urgency>]) -> bool {
    urgencies.is_empty()
        || urgencies
            .iter()
            .any(|u| *u == Some(Urgency::High) || u.is_none())
}

/// Scores the session's warning and error notices in the background
/// while a use of the seam runs.
pub(crate) fn watch_notices(core: &Arc<SessionCore>) {
    let session = Arc::downgrade(core);
    core.events.watch_notices(Box::new(move |level, text| {
        if level == NoticeLevel::Info {
            return;
        }
        let Some(core) = session.upgrade() else {
            return;
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        if active(&core, USES, Seam::Attention).is_empty() {
            return;
        }
        let item = AttentionItem::notice(level, text);
        runtime.spawn(async move {
            let cancel = core.cancel.clone();
            score_attention(&core, vec![item], &cancel).await;
        });
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_fire_unless_every_item_scored_below_high() {
        assert!(worth_notifying(&[]));
        assert!(worth_notifying(&[None]));
        assert!(worth_notifying(&[Some(Urgency::Low), Some(Urgency::High)]));
        assert!(worth_notifying(&[Some(Urgency::Low), None]));
        assert!(!worth_notifying(&[
            Some(Urgency::Low),
            Some(Urgency::Normal)
        ]));
    }

    #[test]
    fn notices_are_keyed_by_their_text() {
        let item = AttentionItem::notice(NoticeLevel::Error, "disk full");
        assert_eq!(item.key, "notice:disk full");
        assert_eq!(item.text, "error: disk full");
        assert_eq!(item.kind.label(), "notice");
    }
}
