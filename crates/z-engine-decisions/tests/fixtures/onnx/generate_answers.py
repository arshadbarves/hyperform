# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=2", "onnxruntime>=1.20", "transformers>=4.48"]
# ///
"""Regenerates answers_<checkpoint>.json: Laya's own ONNXAgent on a downloaded model.

    git clone --branch v0.3.22 https://github.com/NandhaKishorM/laya /tmp/laya
    uv run crates/z-engine-decisions/tests/fixtures/onnx/generate_answers.py /tmp/laya \
        <data dir>/models/laya/<revision> multilingual

The model folder is the one the app downloads (onnx-community export, pinned revision). Laya
reads `rl_agent_config.json` and `tokenizer/`, so they are staged in a temporary folder from the
export's `config.json` (`laya` section) and tokenizer files; the graph is used where it is.
torch is stubbed: ONNXAgent never calls it. `onnx_parity.rs` compares the native runtime with
these answers (Laya v0.3.22, ONNXAgent; Copyright Convai Innovations, Apache-2.0).
"""
import json
import shutil
import sys
import tempfile
from pathlib import Path

from generate import internal, q, stub_torch

HERE = Path(__file__).resolve().parent

GREETING = q("yes_no", "Is the message a greeting?", ("yes", "It greets someone."), ("no", "It does not greet anyone."))
TOOL = q("choice", "Which tool does the request need?", ("edit", "Change a file."), ("read", "Read a file."),
         ("shell", "Run a command in the shell."))
RISK = q("score", "How risky is running this command?", ("none", "Harmless, read-only."),
         ("some", "Changes files that can be restored."), ("high", "Destroys data or cannot be undone."))

CASES = [
    ("greeting", {"text": "Hello there, good morning!"}, GREETING),
    ("not_a_greeting", {"text": "The build failed with three type errors in parser.rs."}, GREETING),
    ("german_greeting", {"text": "Guten Morgen, wie geht es dir?"}, GREETING),
    ("tool_shell", {"request": "run the whole test suite and tell me what fails"}, TOOL),
    ("tool_read", {"request": "show me what src/main.rs contains"}, TOOL),
    ("risk_rm", {"command": "rm -rf ~/projects"}, RISK),
    ("risk_ls", {"command": "ls -la"}, RISK),
    ("history_list", ["user: hi", "assistant: hello, how can I help?", "user: open the config file"], TOOL),
]


def numpy_torch():
    """The tensor calls `collate_items` makes, on numpy (ONNXAgent then calls `.numpy()`)."""
    import numpy as np

    class Tensor(np.ndarray):
        def numpy(self):
            return np.asarray(self)

    torch = sys.modules["torch"]
    torch.long, torch.bool, torch.float32 = np.int64, np.bool_, np.float32
    torch.full = lambda shape, value, dtype=None: np.full(shape, value, dtype=dtype).view(Tensor)
    torch.zeros = lambda shape, dtype=None: np.zeros(shape, dtype=dtype).view(Tensor)
    torch.tensor = lambda data, dtype=None: np.asarray(data, dtype=dtype).view(Tensor)


def main():
    laya_checkout, model_dir, checkpoint = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
    import transformers.tokenization_utils_fast  # noqa: F401  (decides "no torch" before the stub exists)

    stub_torch()
    numpy_torch()
    sys.path.insert(0, str(laya_checkout))
    from laya.onnx_agent import ONNXAgent

    config = json.loads((model_dir / "config.json").read_text())["laya"]
    with tempfile.TemporaryDirectory() as staged:
        staged = Path(staged)
        (staged / "rl_agent_config.json").write_text(json.dumps(config))
        (staged / "tokenizer").mkdir()
        for name in ("tokenizer.json", "tokenizer_config.json"):
            shutil.copy(model_dir / name, staged / "tokenizer" / name)
        agent = ONNXAgent(str(staged), onnx_path=str(model_dir / "onnx" / "model.onnx"))
        max_len = min(1024, {"english": 512, "multilingual": 8192, "typed-decisions": 1024}[checkpoint])
        cases = []
        for name, state, question in CASES:
            laya = internal(question)
            wire = {"type": laya["t"], "instructions": laya["ins"], "criteria": laya["crit"]}
            result = agent._infer(state, {"q": wire}, max_len=max_len)
            answer = result["answers"]["q"]
            answer.pop("action", None)
            answer.pop("legend", None)
            cases.append({"name": name, "state": state, "question": question, "answer": answer})
    out = {"laya": "v0.3.22", "runtime": "ONNXAgent", "checkpoint": checkpoint, "max_len": max_len,
           "generator": "generate_answers.py", "cases": cases}
    (HERE / f"answers_{checkpoint}.json").write_text(json.dumps(out, ensure_ascii=False, indent=1) + "\n")


if __name__ == "__main__":
    main()
