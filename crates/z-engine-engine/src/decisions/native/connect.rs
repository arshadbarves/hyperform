//! `decisions.runtime = "native"`: the pinned model in this process, when
//! the app was built with the `onnx` feature and the model is downloaded.
//! Anything missing is a reason, and the caller falls back to rules.

use std::time::Duration;

use z_engine_config::DecisionSettings;

use crate::decisions::build::Connection;
use crate::session::Shared;

#[cfg(not(feature = "onnx"))]
const NOT_BUILT: &str = "this app was built without the native runtime (the `onnx` feature); use \
     the sidecar runtime";

#[cfg(not(feature = "onnx"))]
pub(crate) async fn connect(
    _shared: &Shared,
    _decisions: &DecisionSettings,
    _timeout: Duration,
) -> Result<Connection, String> {
    Err(NOT_BUILT.into())
}

#[cfg(feature = "onnx")]
pub(crate) async fn connect(
    shared: &Shared,
    decisions: &DecisionSettings,
    timeout: Duration,
) -> Result<Connection, String> {
    use z_engine_decisions::OnnxConfig;

    use super::files::{bytes_on_disk, model_dir, resolve, specs};

    let model = resolve(&decisions.checkpoint)?;
    let dir = model_dir(&shared.paths, model);
    if bytes_on_disk(&specs(&dir, model)).await < model.total_size() {
        return Err(format!(
            "the native model for checkpoint \"{}\" is not downloaded; download it in Settings \
             (Experimental, Decision model)",
            model.checkpoint
        ));
    }
    let max_batch = decisions.max_batch as usize;
    let config = OnnxConfig::for_model(model, dir.clone(), decisions.max_len, timeout, max_batch);
    Ok(Connection {
        provider: shared.sidecars.native().provider(config),
        endpoint: format!("native: {}", dir.display()),
        sidecar: None,
    })
}
