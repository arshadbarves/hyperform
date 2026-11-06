# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy>=2"]
# ///
"""Regenerates sequences.json and decoding.json from Laya's reference Python.

    git clone --branch v0.3.22 https://github.com/NandhaKishorM/laya /tmp/laya
    uv run crates/z-engine-decisions/tests/fixtures/onnx/generate.py /tmp/laya [config.json]

The optional `config.json` is the English checkpoint's (onnx-community/laya-ONNX); its
`laya` temperatures add decoding cases with the shipped values.

Runs Laya's own `build_sequence`, `render_options` and `ONNXAgent._decode_answers`
(laya/common.py, laya/onnx_agent.py; Copyright Convai Innovations, Apache-2.0) with
torch stubbed out, since none of them touch it. Sequences use a stand-in tokenizer the
Rust tests reimplement: whitespace-led word pieces (`\\s*\\S+`), each id
`10 + fnv1a32(utf-8 piece) % 50000`; cls 1, sep 2, mask 3 (`[MASK]`), pad 0. Dict states
keep sorted keys, the order serde_json gives them.
"""
import json
import re
import sys
import types
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent


class _Stub:
    def __init__(self, *args, **kwargs):
        pass

    def __call__(self, *args, **kwargs):
        return args[0] if args else self


class _StubModule(types.ModuleType):
    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        return type(name, (_Stub,), {})


def stub_torch():
    for name in ["torch", "torch.nn", "torch.nn.functional", "torch.utils", "torch.utils.checkpoint"]:
        sys.modules[name] = _StubModule(name)
    sys.modules["torch"].nn = sys.modules["torch.nn"]
    sys.modules["torch.nn"].functional = sys.modules["torch.nn.functional"]
    sys.modules["torch"].utils = sys.modules["torch.utils"]
    sys.modules["torch.utils"].checkpoint = sys.modules["torch.utils.checkpoint"]


def fnv1a32(text):
    h = 0x811C9DC5
    for byte in text.encode("utf-8"):
        h ^= byte
        h = (h * 0x01000193) & 0xFFFFFFFF
    return h


class FakeTok:
    mask_token = "[MASK]"
    cls_token_id, sep_token_id, mask_token_id, pad_token_id = 1, 2, 3, 0

    def __call__(self, text, add_special_tokens=True, truncation=False, max_length=None):
        assert add_special_tokens is False
        ids = [10 + fnv1a32(piece) % 50000 for piece in re.findall(r"\s*\S+", text)]
        return {"input_ids": ids[:max_length] if truncation else ids}


def internal(question):
    """z-engine's question (as the Rust provider sends it) in Laya's internal form."""
    form, options = question["form"], question["options"]
    if form == "yes_no":
        return {"t": "choice", "ins": question["instructions"], "crit": {"a": options[0][1], "b": options[1][1]}}
    if form == "score":
        return {"t": "score", "ins": question["instructions"], "crit": [desc for _, desc in options]}
    return {"t": "choice", "ins": question["instructions"], "crit": {key: desc for key, desc in options}}


WORDS = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu".split()


def words(n, offset=0):
    return " ".join(WORDS[(offset + i) % len(WORDS)] + str(i) for i in range(n))


def q(form, instructions, *options):
    return {"form": form, "instructions": instructions, "options": [list(o) for o in options]}


TOOL = q("choice", "Which tool fits the request?", ("read", "Read a file."), ("edit", "Change a file."),
         ("shell", "Run a command."))
YES_NO = q("yes_no", "Is the message a greeting?", ("yes", "It greets."), ("no", "It does not greet."))
SCORE = q("score", "How risky is the command?", ("none", "Harmless."), ("low", "Small risk."),
          ("mid", "Some risk."), ("high", "Large risk."), ("severe", "Destructive."))

SEQUENCE_CASES = [
    ("choice_dict_state", {"path": "src/main.rs", "request": "show me main"}, TOOL, 512, 192),
    ("yes_no_as_neutral_choice", {"text": "Hello there, good morning!"}, YES_NO, 512, 192),
    ("score_nested_floats_unicode", {"a": [1, 2.0, 1.5e-05, 1e16, None, True], "b": {"c": "é ü \"q\" \\ \n"}},
     SCORE, 512, 192),
    ("undescribed_option", "plain text state", q("choice", "Pick one.", ("one", ""), ("two", "The second.")), 512, 192),
    ("mask_text_is_replaced", "a [MASK] in the state",
     q("choice", "Is [MASK] here?", ("x", "has [MASK] inside"), ("y", "clean")), 512, 192),
    ("list_state_keeps_newest", [words(5, i) for i in range(30)], TOOL, 64, 32),
    ("text_state_keeps_first", words(200), TOOL, 64, 32),
    ("long_option_capped_at_48", "s", q("choice", "Long?", ("long", words(80)), ("short", "Short.")), 512, 192),
    ("options_over_head_budget", "s",
     q("choice", "Crowded?", ("a", words(20)), ("b", words(20, 3)), ("c", words(20, 5)), ("d", words(20, 7))),
     512, 40),
    ("long_instructions_cut", "s", q("choice", words(300), ("a", "A."), ("b", "B.")), 512, 64),
    ("no_room_for_state", words(40), q("choice", "Tight?", ("a", "A."), ("b", "B.")), 16, 64),
    ("markers_past_max_len", "s",
     q("choice", "Many?", *[(f"k{i}", words(6, i)) for i in range(8)]), 24, 64),
]


def sequences(common):
    tok, cases = FakeTok(), []
    for name, state, question, max_len, head_max_len in SEQUENCE_CASES:
        laya = internal(question)
        state_ids = tok(common.serialize_state(state).replace(tok.mask_token, " "), add_special_tokens=False)["input_ids"]
        ids, markers = common.build_sequence(tok, state, laya, max_len, head_max_len,
                                             truncate_left=isinstance(state, list), state_ids=state_ids)
        case = {"name": name, "state": state, "question": question, "max_len": max_len,
                "head_max_len": head_max_len, "options": common.render_options(laya),
                "state_text": common.serialize_state(state), "state_ids": state_ids}
        if len(markers) != len(case["options"]):
            case["error"] = True
        else:
            case.update(ids=ids, markers=markers)
        cases.append(case)
    return cases


def decoding(common, onnx_agent, temps):
    flat = {"temperature": [1.0, 1.0, 1.0], "temperature_by_options": {}}
    odd = {"temperature": [9.0, "x", 0.1], "temperature_by_options": {"choice:3-5": 1.7, "choice:11+": 0.1006}}
    many = q("choice", "Many?", *[(f"k{i}", "") for i in range(12)])
    cases = [
        ("yes_no_flat", YES_NO, [0.3, 2.1], flat),
        ("tool_flat_ties_take_first", TOOL, [1.0, 1.0, 0.5], flat),
        ("tool_odd_bucket", TOOL, [0.2, 1.4, -0.7], odd),
        ("score_odd_type", SCORE, [-1.0, 0.5, 2.0, 0.1, -3.0], odd),
        ("score_flat", SCORE, [0.0, 0.0, 0.0, 0.0, 0.0], flat),
        ("many_options_clamped_sharpener", many, [0.1 * i for i in range(12)], odd),
        ("yes_no_large_logits", YES_NO, [12.5, -8.25], flat),
    ]
    if temps:
        cases += [
            ("tool_english_temps", TOOL, [0.9, -0.4, 0.2], temps),
            ("score_english_temps", SCORE, [0.1, 0.9, 1.7, 0.3, -0.5], temps),
            ("yes_no_english_temps", YES_NO, [-0.6, 0.4], temps),
        ]
    out = []
    for name, question, logits, config in cases:
        laya = internal(question)
        agent = types.SimpleNamespace(
            temperature=[common.clamp_temperature(t) for t in config["temperature"]],
            temperature_by_options={k: common.clamp_temperature(v) for k, v in config["temperature_by_options"].items()},
            lang_temperatures={},
        )
        k = len(logits)
        row = np.full((1, k + 2), -1e4, dtype=np.float32)  # padded slots, as a batch would hold them
        row[0, :k] = np.asarray(logits, dtype=np.float32)
        items = [{"markers": list(range(k))}]
        act = np.zeros((1, 2), dtype=np.float32)
        answer = onnx_agent.ONNXAgent._decode_answers(agent, row, act, items, ["q"], {"q": laya}, 0)["q"]
        out.append({"name": name, "question": question, "logits": row[0].tolist(), "markers": k,
                    "temperature": config["temperature"],
                    "temperature_by_options": config["temperature_by_options"], "answer": answer})
    return out


def main():
    checkout = Path(sys.argv[1]).resolve()
    stub_torch()
    sys.path.insert(0, str(checkout))
    from laya import common, onnx_agent

    temps = None
    if len(sys.argv) > 2:
        laya = json.loads(Path(sys.argv[2]).read_text())["laya"]
        temps = {"temperature": laya["temperature"], "temperature_by_options": laya["temperature_by_options"]}
    source = {"laya": "v0.3.22", "generator": "generate.py"}
    (HERE / "sequences.json").write_text(json.dumps({**source, "cases": sequences(common)}, ensure_ascii=False, indent=1) + "\n")
    (HERE / "decoding.json").write_text(json.dumps({**source, "cases": decoding(common, onnx_agent, temps)}, ensure_ascii=False, indent=1) + "\n")


if __name__ == "__main__":
    main()
