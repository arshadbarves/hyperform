//! The decision layer: typed questions answered by a small local model
//! (Laya through laya-serve, Jev, or in-process through ONNX Runtime with
//! the `onnx` feature), with rules as the fallback. The model proposes and
//! the engine decides; any failure means "keep today's behavior". The only
//! network this crate does is the SystemOne request; the native runtime
//! reads the model files the engine downloaded.

mod answer;
mod cache;
mod calibration;
mod error;
mod native_model;
mod probe;
mod provider;
mod question;
mod trace;

pub mod providers;

pub use answer::{AbstainReason, Answer, Verdict};
pub use cache::{CACHE_SCHEMA_VERSION, DecisionCache, cache_key};
pub use calibration::{Calibration, QuestionCalibration};
pub use error::DecisionError;
pub use native_model::{
    MODEL_CONFIG, MODEL_GRAPH, MODEL_TOKENIZER, MODEL_TOKENIZER_CONFIG, ModelFile, NATIVE_MODELS,
    NativeModel, native_model,
};
pub use probe::{ProbeReport, probe};
pub use provider::DecisionProvider;
pub use providers::{HybridProvider, RulesProvider, SystemOneConfig, SystemOneProvider};
#[cfg(feature = "onnx")]
pub use providers::{OnnxConfig, OnnxProvider};
pub use question::{Criterion, DecisionRequest, Form, MAX_OPTIONS, Question};
pub use trace::{DecisionRecord, DecisionSummary, DecisionTrace, UNCHANGED};
