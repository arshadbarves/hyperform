//! The engine's event sink: every envelope of every session goes to the
//! webview on one channel.

use std::sync::Arc;

use hyperform_engine::EventSink;
use tauri::{AppHandle, Emitter};

pub(crate) const ENGINE_EVENT: &str = "engineEvent";

pub(crate) fn sink(app: AppHandle) -> EventSink {
    Arc::new(move |envelope| {
        if let Err(error) = app.emit(ENGINE_EVENT, envelope) {
            tracing::warn!(%error, "engine event not delivered");
        }
    })
}
