//! Seam: a main-agent turn ended. Uses run in a background task so the
//! turn ends at once; a tone from an `on` use reaches the GUI as
//! `TurnToneJudged`, where the pet's mood may use it.

use std::future::Future;
use std::sync::Arc;

use z_engine_protocol::{Event, TurnRecord};

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::session::SessionCore;

pub(crate) fn at_turn_end(core: &Arc<SessionCore>, turn: &TurnRecord) {
    if let Some(work) = turn_end_work(USES, core, turn) {
        tokio::spawn(work);
    }
}

/// The background work, or `None` when no use of the seam runs.
pub(crate) fn turn_end_work(
    uses: &[&'static dyn DecisionUse],
    core: &Arc<SessionCore>,
    turn: &TurnRecord,
) -> Option<impl Future<Output = ()> + Send + 'static> {
    let active = active(core, uses, Seam::TurnEnd);
    if active.is_empty() {
        return None;
    }
    let core = Arc::clone(core);
    let turn = Arc::new(turn.clone());
    Some(async move {
        let cancel = core.cancel.clone();
        let seen = Arc::clone(&turn);
        let tones = dispatch(&core, active, &cancel, move |decision_use, cx| {
            let turn = Arc::clone(&seen);
            Box::pin(async move { decision_use.turn_end(&cx, &turn).await })
        })
        .await;
        if let Some(tone) = tones.into_iter().flatten().next() {
            core.events.emit(Event::TurnToneJudged {
                turn_id: turn.turn_id.clone(),
                tone,
            });
        }
    })
}
