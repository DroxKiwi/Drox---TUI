//! Newtypes d'identifiants. Évite de confondre une `SessionId` avec une
//! `MessageId` à la compilation.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::now_v7()))
            }

            #[must_use]
            pub const fn from_string(value: String) -> Self {
                Self(value)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

define_id!(SessionId, "ses");
define_id!(MessageId, "msg");
define_id!(AgentId, "agt");
define_id!(ToolUseId, "tu");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_have_distinct_prefixes() {
        let s = SessionId::new();
        let m = MessageId::new();
        assert!(s.as_str().starts_with("ses_"));
        assert!(m.as_str().starts_with("msg_"));
    }
}
