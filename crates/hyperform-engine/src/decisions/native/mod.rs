//! The native runtime in the engine: the pinned model's files under
//! `<data dir>/models/laya/<revision>/`, their download (only ever started
//! from Settings), and, with the `onnx` feature, the loaded model every
//! session shares.

mod connect;
mod files;
mod runtime;

pub(crate) use connect::connect;
pub(crate) use files::{bytes_on_disk, model_dir, resolve, specs};
pub(crate) use runtime::NativeRuntime;
