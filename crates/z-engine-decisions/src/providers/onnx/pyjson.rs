//! Python's `json.dumps(value, ensure_ascii=False)`, which Laya uses to turn
//! a JSON state into the text the model reads.
//!
//! Ported from Laya's `laya/common.py` (`serialize_state`),
//! <https://github.com/NandhaKishorM/laya>, Copyright Convai Innovations,
//! Apache-2.0, and CPython's `json` encoder: `", "` and `": "` separators,
//! keys in the map's order, only `"`, `\` and control characters escaped,
//! and floats written the way Python's `repr` writes them.

use serde_json::{Number, Value};

/// A string state is read as it is; anything else as Python's JSON text.
pub(super) fn serialize_state(state: &Value) -> String {
    match state {
        Value::String(text) => text.clone(),
        other => dumps(other),
    }
}

pub(super) fn dumps(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => write_number(out, number),
        Value::String(text) => write_string(out, text),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (index, (key, item)) in map.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_string(out, key);
                out.push_str(": ");
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

fn write_number(out: &mut String, number: &Number) {
    match number.as_f64().filter(|_| number.is_f64()) {
        Some(float) => out.push_str(&float_repr(float)),
        None => out.push_str(&number.to_string()),
    }
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ch if u32::from(ch) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(ch))),
            ch => out.push(ch),
        }
    }
    out.push('"');
}

/// Python's `repr(float)`: the shortest digits that round-trip, in fixed
/// notation for decimal exponents -4 to 15 (always with a fraction) and as
/// `d.ddde+XX` otherwise. Rust's `{:e}` gives the same shortest digits.
fn float_repr(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_string();
    }
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    let sign = if value < 0.0 { "-" } else { "" };
    if (-4..16).contains(&exponent) {
        let point = exponent + 1;
        let body = if point <= 0 {
            format!("0.{}{digits}", "0".repeat(point.unsigned_abs() as usize))
        } else if point as usize >= digits.len() {
            format!("{digits}{}.0", "0".repeat(point as usize - digits.len()))
        } else {
            let (whole, fraction) = digits.split_at(point as usize);
            format!("{whole}.{fraction}")
        };
        return format!("{sign}{body}");
    }
    let (first, rest) = digits.split_at(1);
    let fraction = if rest.is_empty() {
        String::new()
    } else {
        format!(".{rest}")
    };
    let exponent_sign = if exponent < 0 { '-' } else { '+' };
    format!(
        "{sign}{first}{fraction}e{exponent_sign}{:02}",
        exponent.abs()
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn floats_follow_python_repr() {
        let cases = [
            (1.0, "1.0"),
            (0.1, "0.1"),
            (1e-4, "0.0001"),
            (1e-5, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1.2345678901234568e17, "1.2345678901234568e+17"),
            (-2.5, "-2.5"),
            (-0.0, "-0.0"),
            (123456.789, "123456.789"),
        ];
        for (value, want) in cases {
            assert_eq!(float_repr(value), want, "{value}");
        }
    }

    #[test]
    fn values_use_python_separators_and_escapes() {
        let value = json!({ "a": [1, 2.0, null, true], "b": "é \"q\" \\ \n\t\u{1}" });
        assert_eq!(
            dumps(&value),
            "{\"a\": [1, 2.0, null, true], \"b\": \"é \\\"q\\\" \\\\ \\n\\t\\u0001\"}"
        );
        assert_eq!(serialize_state(&json!("plain text")), "plain text");
        assert_eq!(serialize_state(&json!([])), "[]");
    }
}
