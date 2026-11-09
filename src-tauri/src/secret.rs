//! `Secret` – a string wrapper for passwords, passphrases and private keys.
//!
//! * `Debug` / `Display` never print the value (log redaction by construction).
//! * Memory is zeroed on drop.
//! * Deserializable so it can arrive from the frontend, but intentionally NOT
//!   serializable: secrets never travel back to the frontend.

use serde::{Deserialize, Deserializer};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Secret::new(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let s = Secret::new("hunter2");
        assert_eq!(format!("{s:?}"), "Secret(***)");
        assert!(!format!("{:?}", Some(s.clone())).contains("hunter2"));
        assert_eq!(s.expose(), "hunter2");
    }

    #[test]
    fn deserializes_from_json_string() {
        let s: Secret = serde_json::from_str("\"abc\"").unwrap();
        assert_eq!(s.expose(), "abc");
    }
}
