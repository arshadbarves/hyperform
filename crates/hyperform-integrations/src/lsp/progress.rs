//! Work-done progress a server reports (`$/progress` begin/end, e.g.
//! indexing): while any is open the server is busy, and callers that only
//! want diagnostics opportunistically do not wait for it.

use std::collections::HashSet;
use std::sync::Mutex;

use serde_json::Value;

use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct ProgressTracker {
    open: Mutex<HashSet<String>>,
}

impl ProgressTracker {
    /// Records one `$/progress` notification.
    pub(crate) fn update(&self, params: Option<&Value>) {
        let Some(params) = params else { return };
        let token = match params.get("token") {
            Some(Value::String(token)) => token.clone(),
            Some(Value::Number(token)) => token.to_string(),
            _ => return,
        };
        let kind = params
            .get("value")
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str);
        let mut open = lock(&self.open);
        match kind {
            Some("begin") => {
                open.insert(token);
            }
            Some("end") => {
                open.remove(&token);
            }
            _ => {}
        }
    }

    pub(crate) fn busy(&self) -> bool {
        !lock(&self.open).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn busy_between_begin_and_end_of_any_token() {
        let tracker = ProgressTracker::default();
        let step = |token: Value, kind: &str| {
            tracker.update(Some(&json!({"token": token, "value": {"kind": kind}})));
        };
        assert!(!tracker.busy());
        step(json!("index"), "begin");
        step(json!(7), "begin");
        step(json!("index"), "report");
        step(json!("index"), "end");
        assert!(tracker.busy(), "token 7 is still open");
        step(json!(7), "end");
        assert!(!tracker.busy());
        tracker.update(Some(&json!({"value": {"kind": "begin"}})));
        tracker.update(None);
        assert!(!tracker.busy());
    }
}
