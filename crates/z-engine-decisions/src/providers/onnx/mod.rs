//! The native runtime (the `onnx` feature): Laya's sequence builder and
//! answer decoding ported from its reference Python, over ONNX Runtime and
//! the checkpoint's own tokenizer. The pure port is also built for tests
//! without the feature, so its parity tests never need ONNX Runtime.

#![cfg_attr(not(feature = "onnx"), allow(dead_code))]

mod checkpoint;
mod decode;
#[cfg(feature = "onnx")]
mod provider;
mod pyjson;
#[cfg(feature = "onnx")]
mod runtime;
mod sequence;

#[cfg(test)]
pub(super) mod parity_tests;

#[cfg(feature = "onnx")]
pub use provider::{OnnxConfig, OnnxProvider};

/// The provider's name in answers and traces.
const NAME: &str = "native";
