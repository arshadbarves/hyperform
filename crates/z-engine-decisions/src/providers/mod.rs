//! Decision providers: rules (always abstain), SystemOne (laya-serve or
//! Jev over HTTP), native (Laya in-process, the `onnx` feature) and hybrid
//! (cache, calibration and fallback over a model).

mod hybrid;
#[cfg(any(test, feature = "onnx"))]
mod onnx;
mod rules;
mod systemone;

pub use hybrid::HybridProvider;
#[cfg(feature = "onnx")]
pub use onnx::{OnnxConfig, OnnxProvider};
pub use rules::{RULES, RulesProvider};
pub use systemone::{SystemOneConfig, SystemOneProvider};
