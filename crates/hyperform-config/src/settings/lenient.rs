//! Field adapters accepting every spelling the protocol enums parse (v1 and
//! Claude Code names), so a synonym such as `fmt` for `format` does not
//! reject a whole settings layer. The permission mode reads through
//! `ModeName`, which accepts every spelling itself.

use hyperform_protocol::{CheckKind, Effort, VerificationMode};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

fn parsed<'de, D, T>(
    deserializer: D,
    parse: fn(&str) -> Option<T>,
    what: &str,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    parse(&raw).ok_or_else(|| D::Error::custom(format!("unknown {what} `{raw}`")))
}

pub(crate) fn verification_mode<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<VerificationMode, D::Error> {
    parsed(d, VerificationMode::parse, "verification mode")
}

pub(crate) fn check_kind<'de, D: Deserializer<'de>>(d: D) -> Result<CheckKind, D::Error> {
    parsed(d, CheckKind::parse, "check kind")
}

pub(crate) fn effort<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Effort>, D::Error> {
    match Option::<String>::deserialize(d)? {
        None => Ok(None),
        Some(raw) => Effort::parse(&raw)
            .map(Some)
            .ok_or_else(|| D::Error::custom(format!("unknown effort `{raw}`"))),
    }
}
