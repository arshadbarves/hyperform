//! The engine's event sink: every envelope of every session goes to the
//! webview on one channel.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use z_engine_engine::EventSink;

pub(crate) const ENGINE_EVENT: &str = "engineEvent";

pub(crate) fn sink(app: AppHandle) -> EventSink {
    Arc::new(move |envelope| {
        if let Err(error) = app.emit(ENGINE_EVENT, envelope) {
            tracing::warn!(%error, "engine event not delivered");
        }
    })
}
