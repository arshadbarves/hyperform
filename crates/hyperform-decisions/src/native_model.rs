//! The Laya checkpoints the native runtime can download, pinned to one
//! Hugging Face revision each with the size and SHA-256 of every file.
//!
//! Convai Innovations publishes Laya as safetensors only, so the files are
//! the `onnx-community` exports (Apache-2.0, like Laya): one fp32 graph of
//! Laya's unchanged `DecisionModel.forward` (inputs `input_ids`,
//! `attention_mask`, `marker_pos`, `marker_mask`, `qtype`; outputs `logits`,
//! `act_logits`), the checkpoint's tokenizer, and a `config.json` whose
//! `laya` section carries `max_len`, `head_max_len` and the temperatures of
//! the original `rl_agent_config.json`. Changing a pin means a new revision
//! folder, so a model is never half old and half new.

/// One file of a checkpoint, relative to its revision folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFile {
    pub path: &'static str,
    pub size: u64,
    /// Lowercase hex.
    pub sha256: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeModel {
    /// The `decisions.checkpoint` name laya-serve also accepts.
    pub checkpoint: &'static str,
    /// The Hugging Face repository the files come from.
    pub repo: &'static str,
    /// The pinned commit; names the folder under `<data dir>/models/laya/`.
    pub revision: &'static str,
    /// Longest sequence the checkpoint reads per question; a larger
    /// `decisions.max_len` is cut to it.
    pub max_len_cap: u32,
    pub files: &'static [ModelFile],
}

/// The graph, relative to the revision folder; its weights sit next to it.
pub const MODEL_GRAPH: &str = "onnx/model.onnx";
/// The `laya` section with token budgets and temperatures.
pub const MODEL_CONFIG: &str = "config.json";
pub const MODEL_TOKENIZER: &str = "tokenizer.json";
/// Names the special tokens (`cls`, `sep`, `mask`, `pad`) Laya builds with.
pub const MODEL_TOKENIZER_CONFIG: &str = "tokenizer_config.json";

impl NativeModel {
    /// The download address of `file` at the pinned revision.
    pub fn url(&self, file: &ModelFile) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repo, self.revision, file.path
        )
    }

    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }
}

/// `decisions.checkpoint` (or one of laya's aliases) to its model.
pub fn native_model(checkpoint: &str) -> Option<&'static NativeModel> {
    let name = checkpoint.trim().to_ascii_lowercase();
    let name = match name.as_str() {
        "en" | "laya" | "default" => "english",
        "multi" | "ml" | "laya-multilingual" => "multilingual",
        "typed" | "typed_decisions" | "laya-typed-decisions" | "decisions" => "typed-decisions",
        other => other,
    };
    NATIVE_MODELS.iter().find(|model| model.checkpoint == name)
}

const fn file(path: &'static str, size: u64, sha256: &'static str) -> ModelFile {
    ModelFile { path, size, sha256 }
}

pub const NATIVE_MODELS: &[NativeModel] = &[
    NativeModel {
        checkpoint: "english",
        repo: "onnx-community/laya-ONNX",
        revision: "42e2a6e3b3708c8ff5f61f4aa75c314b1449199b",
        // Trained at 512 tokens; about 320 are left for the state.
        max_len_cap: 512,
        files: &[
            file(
                MODEL_CONFIG,
                3_281,
                "1a888f1373542026f6e09b4bdfa746eb6ed6cd0713187cf18c5602e92993bb0c",
            ),
            file(
                MODEL_TOKENIZER_CONFIG,
                308,
                "50044de60daaa73df97d262e15a40d4faf0160e7d742df64b377877a1320dd12",
            ),
            file(
                MODEL_TOKENIZER,
                3_583_228,
                "6c8aaa9a542084f2457eab775d4eeb51f92a70c0fd9de28d5edb0ddec3c08d30",
            ),
            file(
                MODEL_GRAPH,
                4_442_939,
                "04ac226b2ce6cfd35a28e8cefcc897e9fcc2901b0bfa86d0f518d5b94491f7b0",
            ),
            file(
                "onnx/model.onnx_data",
                1_685_175_296,
                "afcc11e72b59193fbec6e5dc8b693c9baa81cf66bec53279cefd687b318c3199",
            ),
        ],
    },
    NativeModel {
        checkpoint: "multilingual",
        repo: "onnx-community/laya-multilingual-ONNX",
        revision: "46b77bbf5642fec5f14e540570228a8cbe8ab81f",
        max_len_cap: 8_192,
        files: &[
            file(
                MODEL_CONFIG,
                2_859,
                "cc32109b1fded91ec6734127ffb1827f80cdcf8ffdd4eeda75a1b0f4d3687dcc",
            ),
            file(
                MODEL_TOKENIZER_CONFIG,
                502,
                "424b69444bf7b5809dc2cd2e36d0bd71b8055124dd24274d6db3c655d38205e7",
            ),
            file(
                MODEL_TOKENIZER,
                34_363_188,
                "609d8f4c067cd3950f88594c5a802616cea245823836ef5848ee4fc40aab5b6f",
            ),
            file(
                MODEL_GRAPH,
                3_553_251,
                "22461d988f675dd6c2b89ff8feb9ed7ffcf7e37854e5ed512f8b275178cf2f9f",
            ),
            file(
                "onnx/model.onnx_data",
                1_287_635_968,
                "f01238e183ace77661282e4eb2dd9b872c7044cb9678f904cf409e6cc1146d09",
            ),
        ],
    },
    NativeModel {
        checkpoint: "typed-decisions",
        repo: "onnx-community/laya-typed-decisions-ONNX",
        revision: "79c441b22da79eb3727bb01e8452e29a32cfa548",
        max_len_cap: 1_024,
        files: &[
            file(
                MODEL_CONFIG,
                3_300,
                "17276af4594edd2528024d1a23de7905e789f8befe659a47e337aa4ce144a226",
            ),
            file(
                MODEL_TOKENIZER_CONFIG,
                337,
                "08d4cf3ac4dca381759441b85b91a6d40e688471dcd33d15d6649eb0a9a854d1",
            ),
            file(
                MODEL_TOKENIZER,
                3_583_228,
                "6c8aaa9a542084f2457eab775d4eeb51f92a70c0fd9de28d5edb0ddec3c08d30",
            ),
            file(
                MODEL_GRAPH,
                4_442_939,
                "08a09f491df9efe9d5cbc96d8c5f22a3369951b58c9c64b568db588ab66c21d4",
            ),
            file(
                "onnx/model.onnx_data",
                1_685_175_296,
                "92778ebb6a278de5a7041b8a2f9449a05172264d99d8cf471a878ec50ae04741",
            ),
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_and_aliases_resolve() {
        assert_eq!(native_model("multilingual").unwrap().max_len_cap, 8_192);
        assert_eq!(native_model(" EN ").unwrap().checkpoint, "english");
        assert_eq!(native_model("typed").unwrap().checkpoint, "typed-decisions");
        assert!(native_model("gpt").is_none());
    }

    #[test]
    fn every_model_pins_every_file_it_needs() {
        for model in NATIVE_MODELS {
            assert_eq!(model.revision.len(), 40, "{}", model.checkpoint);
            for path in [
                MODEL_GRAPH,
                MODEL_CONFIG,
                MODEL_TOKENIZER,
                MODEL_TOKENIZER_CONFIG,
            ] {
                assert!(model.files.iter().any(|file| file.path == path), "{path}");
            }
            for file in model.files {
                assert_eq!(file.sha256.len(), 64, "{}", file.path);
                assert!(file.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
                assert!(!file.path.starts_with('/') && !file.path.contains(".."));
            }
            assert!(model.total_size() > 1_000_000_000);
        }
        let english = native_model("english").unwrap();
        assert_eq!(
            english.url(&english.files[0]),
            "https://huggingface.co/onnx-community/laya-ONNX/resolve/\
             42e2a6e3b3708c8ff5f61f4aa75c314b1449199b/config.json"
        );
    }
}
