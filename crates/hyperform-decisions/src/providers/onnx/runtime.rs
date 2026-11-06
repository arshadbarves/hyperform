//! The loaded model: an ONNX Runtime session over Laya's graph, the
//! checkpoint's tokenizer, its special tokens, budgets and temperatures.
//! Everything here blocks; the provider runs it off the async runtime, and
//! terminating the `RunOptions` stops a run in progress.
//!
//! Batching and inputs follow Laya's `laya/onnx_agent.py`
//! (`_infer_batch`) and `laya/common.py` (`collate_items`),
//! <https://github.com/NandhaKishorM/laya>, Copyright Convai Innovations,
//! Apache-2.0: one row per question, padded to the longest, markers padded
//! with masked slots.

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use ort::session::builder::GraphOptimizationLevel;
use ort::session::{RunOptions, Session};
use ort::value::Tensor;
use sha2::{Digest, Sha256};
use tokenizers::Tokenizer;

use super::NAME;
use super::checkpoint::{CheckpointConfig, SpecialTexts};
use super::decode::{Temperatures, decode};
use super::provider::OnnxConfig;
use super::pyjson::serialize_state;
use super::sequence::{
    Budget, LayaQuestion, Row, SpecialTokens, Tokenize, build_row, encode_state,
};
use crate::answer::{AbstainReason, Answer};
use crate::error::DecisionError;
use crate::native_model::{MODEL_CONFIG, MODEL_GRAPH, MODEL_TOKENIZER, MODEL_TOKENIZER_CONFIG};
use crate::question::DecisionRequest;

struct Tok(Tokenizer);

impl Tokenize for Tok {
    fn encode(&self, text: &str) -> Result<Vec<u32>, String> {
        let encoding = self
            .0
            .encode(text, false)
            .map_err(|error| error.to_string())?;
        Ok(encoding.get_ids().to_vec())
    }
}

pub(super) struct Runtime {
    session: Mutex<Session>,
    tokenizer: Tok,
    special: SpecialTokens,
    budget: Budget,
    temperatures: Temperatures,
}

impl fmt::Debug for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Runtime")
            .field("special", &self.special)
            .field("budget", &self.budget)
            .finish_non_exhaustive()
    }
}

/// A question that fits, waiting for its session run.
struct Pending<'q> {
    index: usize,
    question: &'q LayaQuestion,
    row: Row,
}

impl Runtime {
    /// Verifies every pinned file, then loads the tokenizer and the graph.
    pub(super) fn load(config: &OnnxConfig) -> Result<Self, String> {
        let dir = config.dir.as_path();
        verify(dir, &config.expected_sha256)?;
        let checkpoint = CheckpointConfig::parse(&read(dir, MODEL_CONFIG)?)?;
        let texts = SpecialTexts::parse(&read(dir, MODEL_TOKENIZER_CONFIG)?)?;
        let mut tokenizer = Tokenizer::from_file(dir.join(MODEL_TOKENIZER))
            .map_err(|error| format!("{MODEL_TOKENIZER} could not be read: {error}"))?;
        tokenizer
            .with_truncation(None)
            .map_err(|error| error.to_string())?;
        tokenizer.with_padding(None);
        let id = |text: &str| {
            tokenizer
                .token_to_id(text)
                .ok_or_else(|| format!("the tokenizer has no `{text}` token"))
        };
        let special = SpecialTokens {
            cls: id(&texts.cls)?,
            sep: id(&texts.sep)?,
            mask: id(&texts.mask)?,
            pad: id(&texts.pad)?,
            mask_text: texts.mask.clone(),
        };
        let session = Session::builder()
            .and_then(|builder| builder.with_optimization_level(GraphOptimizationLevel::Level3))
            .and_then(|builder| builder.commit_from_file(dir.join(MODEL_GRAPH)))
            .map_err(|error| format!("ONNX Runtime could not load the model: {error}"))?;
        Ok(Self {
            session: Mutex::new(session),
            tokenizer: Tok(tokenizer),
            special,
            budget: Budget {
                max_len: config.max_len as usize,
                head_max_len: checkpoint.head_max_len,
            },
            temperatures: checkpoint.temperatures,
        })
    }

    /// One answer per question, in order. A question whose options do not
    /// fit abstains; a failed run fails the call.
    pub(super) fn answer(
        &self,
        request: &DecisionRequest,
        max_batch: usize,
        run: &RunOptions,
    ) -> Result<Vec<Answer>, DecisionError> {
        let state = serialize_state(&request.state);
        let state_ids = encode_state(&self.tokenizer, &self.special, &state)
            .map_err(DecisionError::Malformed)?;
        let truncate_left = request.state.is_array();
        let questions: Vec<LayaQuestion> = request.questions.iter().map(LayaQuestion::of).collect();
        let mut answers: Vec<Option<Answer>> = vec![None; questions.len()];
        let mut pending = Vec::with_capacity(questions.len());
        for (index, question) in questions.iter().enumerate() {
            let name = &request.questions[index].name;
            match build_row(
                &self.tokenizer,
                &self.special,
                question,
                &state_ids,
                self.budget,
                truncate_left,
            ) {
                Ok(row) => pending.push(Pending {
                    index,
                    question,
                    row,
                }),
                Err(reason) => {
                    tracing::debug!(question = %name, %reason, "question does not fit the native model");
                    answers[index] = Some(Answer::abstained(name, AbstainReason::Invalid, NAME));
                }
            }
        }
        for chunk in pending.chunks(max_batch.max(1)) {
            let (width, logits) = self.run(chunk, run)?;
            for (offset, item) in chunk.iter().enumerate() {
                let line = &logits[offset * width..(offset + 1) * width];
                let markers = item.row.markers.len().min(width);
                let question = &request.questions[item.index];
                let answer = decode(
                    question,
                    item.question,
                    &line[..markers],
                    &self.temperatures,
                );
                answers[item.index] = Some(answer);
            }
        }
        let names = request.questions.iter().map(|question| &question.name);
        Ok(answers
            .into_iter()
            .zip(names)
            .map(|(answer, name)| {
                answer.unwrap_or_else(|| Answer::abstained(name, AbstainReason::Malformed, NAME))
            })
            .collect())
    }

    /// One session run over `rows`; the logits' row width and values.
    fn run(
        &self,
        rows: &[Pending<'_>],
        run: &RunOptions,
    ) -> Result<(usize, Vec<f32>), DecisionError> {
        let batch = rows.len();
        let seq = rows
            .iter()
            .map(|item| item.row.ids.len())
            .max()
            .unwrap_or(0);
        let width = rows
            .iter()
            .map(|item| item.row.markers.len())
            .max()
            .unwrap_or(0);
        let mut input_ids = vec![i64::from(self.special.pad); batch * seq];
        let mut attention = vec![0_i64; batch * seq];
        let mut marker_pos = vec![0_i64; batch * width];
        let mut marker_mask = vec![false; batch * width];
        let mut qtype = Vec::with_capacity(batch);
        for (r, item) in rows.iter().enumerate() {
            for (c, id) in item.row.ids.iter().enumerate() {
                input_ids[r * seq + c] = i64::from(*id);
                attention[r * seq + c] = 1;
            }
            for (c, marker) in item.row.markers.iter().enumerate() {
                marker_pos[r * width + c] = i64::try_from(*marker).unwrap_or(0);
                marker_mask[r * width + c] = true;
            }
            qtype.push(item.question.kind as i64);
        }
        let fail = |error: ort::Error| {
            DecisionError::Unavailable(format!("the native model failed: {error}"))
        };
        let inputs = ort::inputs! {
            "input_ids" => Tensor::from_array(([batch, seq], input_ids)).map_err(fail)?,
            "attention_mask" => Tensor::from_array(([batch, seq], attention)).map_err(fail)?,
            "marker_pos" => Tensor::from_array(([batch, width], marker_pos)).map_err(fail)?,
            "marker_mask" => Tensor::from_array(([batch, width], marker_mask)).map_err(fail)?,
            "qtype" => Tensor::from_array(([batch], qtype)).map_err(fail)?,
        };
        let mut session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        let outputs = session.run_with_options(inputs, run).map_err(fail)?;
        let (shape, values) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(fail)?;
        let columns = shape.last().copied().unwrap_or(0);
        let columns = usize::try_from(columns).unwrap_or(0);
        if columns < width || values.len() != batch * columns {
            return Err(DecisionError::Malformed(format!(
                "the native model returned logits of shape {shape:?} for {batch} rows of {width} options"
            )));
        }
        Ok((columns, values.to_vec()))
    }
}

fn read(dir: &Path, name: &str) -> Result<String, String> {
    std::fs::read_to_string(dir.join(name))
        .map_err(|error| format!("{name} could not be read: {error}"))
}

/// Laya's `verify_digests`: every listed file must exist and match before
/// any of them is parsed.
fn verify(dir: &Path, expected: &[(String, String)]) -> Result<(), String> {
    for (path, want) in expected {
        let got = sha256_file(&dir.join(path))
            .map_err(|error| format!("{path} of the native model could not be read: {error}"))?;
        if !got.eq_ignore_ascii_case(want) {
            return Err(format!(
                "{path} of the native model does not match its pinned SHA-256; remove the model \
                 in Settings and download it again"
            ));
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
