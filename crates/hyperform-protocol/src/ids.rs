//! Opaque identifiers. Generated ids are time-ordered ULIDs; most carry a
//! short prefix so logs stay readable. Session ids are bare ULIDs so v1
//! session files keep their identity after import.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! prefixed_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
        #[serde(transparent)]
        #[ts(export)]
        pub struct $name(pub String);

        impl $name {
            /// A fresh, time-ordered identifier.
            pub fn new() -> Self {
                Self(format!(
                    concat!($prefix, "_{}"),
                    ulid::Ulid::new().to_string().to_lowercase()
                ))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }
    };
}

prefixed_id!(
    /// One agent run (the main agent or a spawned subagent).
    AgentId,
    "agt"
);
prefixed_id!(
    /// A model tool call. Provider-issued ids are kept verbatim.
    CallId,
    "call"
);
prefixed_id!(
    /// A code checkpoint taken before a user turn.
    CheckpointId,
    "ckp"
);
prefixed_id!(
    /// A background job: a shell process or a background agent.
    JobId,
    "job"
);
prefixed_id!(
    /// A persisted conversation message.
    MessageId,
    "msg"
);
prefixed_id!(
    /// A pending user interaction: approval, question, or plan review.
    RequestId,
    "req"
);
prefixed_id!(
    /// One user turn of the main agent.
    TurnId,
    "trn"
);
prefixed_id!(
    /// A persistent work plan of one project.
    PlanId,
    "pln"
);
prefixed_id!(
    /// One task of a work plan.
    TaskId,
    "tsk"
);
prefixed_id!(
    /// A plan item (requirement or invariant) or a ledger entry.
    EntryId,
    "ent"
);

impl AgentId {
    /// The session's root agent.
    pub fn main() -> Self {
        Self("main".to_string())
    }

    pub fn is_main(&self) -> bool {
        self.0 == "main"
    }
}

/// A conversation session (bare ULID, compatible with v1 file names).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SessionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_prefixed_and_unique() {
        let a = MessageId::new();
        let b = MessageId::new();
        assert!(a.as_str().starts_with("msg_"));
        assert_ne!(a, b);
        assert_eq!(SessionId::new().as_str().len(), 26);
    }

    #[test]
    fn ids_serialize_as_plain_strings() {
        let id = AgentId::main();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"main\"");
        assert!(id.is_main());
    }
}
