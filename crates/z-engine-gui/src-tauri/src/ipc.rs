//! Command results as the webview receives them: failures become their
//! display text, successes are serialized once.

use std::fmt::Display;

use serde::Serialize;
use serde_json::Value;

pub(crate) type IpcResult<T> = Result<T, String>;

/// Display text of any error, for `map_err`.
pub(crate) fn fail(error: impl Display) -> String {
    error.to_string()
}

/// Serializes engine results whose types are not named outside the engine.
pub(crate) fn json(value: impl Serialize) -> IpcResult<Value> {
    serde_json::to_value(value).map_err(fail)
}
