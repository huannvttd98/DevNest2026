//! Per-connection SSH algorithm options, written like OpenSSH `-o` options:
//!
//! ```text
//! HostKeyAlgorithms=+ssh-rsa
//! PubkeyAcceptedKeyTypes=+ssh-rsa
//! KexAlgorithms=+diffie-hellman-group14-sha1
//! Ciphers=+aes128-cbc
//! MACs=+hmac-sha1
//! ```
//!
//! Value syntax follows `ssh_config(5)`: `+a,b` appends, `-a` removes, `^a` puts first,
//! anything else replaces the default list. Stored as newline-separated text.

use std::borrow::Cow;

use russh::keys::Algorithm;
use russh::{Preferred, cipher, kex, mac};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    HostKeyAlgorithms,
    PubkeyAccepted,
    KexAlgorithms,
    Ciphers,
    Macs,
}

impl Key {
    fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "hostkeyalgorithms" => Some(Key::HostKeyAlgorithms),
            "pubkeyacceptedkeytypes" | "pubkeyacceptedalgorithms" => Some(Key::PubkeyAccepted),
            "kexalgorithms" => Some(Key::KexAlgorithms),
            "ciphers" => Some(Key::Ciphers),
            "macs" => Some(Key::Macs),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    Append(Vec<String>),
    Remove(Vec<String>),
    Prepend(Vec<String>),
    Replace(Vec<String>),
}

#[derive(Debug, Clone)]
struct Entry {
    key: Key,
    op: Op,
}

/// Parsed, validated option set.
#[derive(Debug, Clone, Default)]
pub struct SshOptions {
    entries: Vec<Entry>,
}

pub const SUPPORTED_KEYS: &str = "HostKeyAlgorithms, PubkeyAcceptedKeyTypes (PubkeyAcceptedAlgorithms), KexAlgorithms, Ciphers, MACs";

fn split_list(v: &str) -> Vec<String> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_op(value: &str) -> Op {
    let v = value.trim();
    if let Some(rest) = v.strip_prefix('+') {
        Op::Append(split_list(rest))
    } else if let Some(rest) = v.strip_prefix('-') {
        Op::Remove(split_list(rest))
    } else if let Some(rest) = v.strip_prefix('^') {
        Op::Prepend(split_list(rest))
    } else {
        Op::Replace(split_list(v))
    }
}

fn names(op: &Op) -> &[String] {
    match op {
        Op::Append(n) | Op::Remove(n) | Op::Prepend(n) | Op::Replace(n) => n,
    }
}

fn parse_host_key_alg(name: &str) -> Option<Algorithm> {
    match name {
        // ssh-key parses "ssh-rsa" as RSA with no hash = legacy SHA-1 signatures.
        "ssh-rsa" => Some(Algorithm::Rsa { hash: None }),
        other => Algorithm::new(other).ok(),
    }
}

fn kex_name(name: &str) -> Option<kex::Name> {
    kex::Name::try_from(name).ok()
}
fn cipher_name(name: &str) -> Option<cipher::Name> {
    cipher::Name::try_from(name).ok()
}
fn mac_name(name: &str) -> Option<mac::Name> {
    mac::Name::try_from(name).ok()
}

impl SshOptions {
    /// Parse newline-separated `Key=Value` (or `Key Value`) lines. Empty lines and
    /// `#` comments are ignored. Unknown keys or algorithm names are rejected.
    pub fn parse(text: &str) -> AppResult<Self> {
        let mut entries = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (k, v) = line
                .split_once('=')
                .or_else(|| line.split_once(char::is_whitespace))
                .ok_or_else(|| {
                    AppError::validation(format!("SSH option line {}: expected Key=Value", i + 1))
                })?;
            let key = Key::parse(k.trim()).ok_or_else(|| {
                AppError::validation(format!(
                    "SSH option '{}' is not supported. Supported: {SUPPORTED_KEYS}",
                    k.trim()
                ))
            })?;
            let op = parse_op(v);
            if names(&op).is_empty() {
                return Err(AppError::validation(format!(
                    "SSH option '{}' has no value",
                    k.trim()
                )));
            }
            for n in names(&op) {
                let ok = match key {
                    Key::HostKeyAlgorithms | Key::PubkeyAccepted => parse_host_key_alg(n).is_some(),
                    Key::KexAlgorithms => kex_name(n).is_some(),
                    Key::Ciphers => cipher_name(n).is_some(),
                    Key::Macs => mac_name(n).is_some(),
                };
                if !ok {
                    return Err(AppError::validation(format!(
                        "Unknown or unsupported algorithm '{n}' for {}",
                        k.trim()
                    )));
                }
            }
            entries.push(Entry { key, op });
        }
        Ok(Self { entries })
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build the russh algorithm preferences.
    pub fn preferred(&self) -> Preferred {
        let mut p = Preferred::default();
        for e in &self.entries {
            match e.key {
                Key::HostKeyAlgorithms => {
                    p.key = Cow::Owned(apply(&p.key, &e.op, parse_host_key_alg));
                }
                Key::KexAlgorithms => {
                    let mut list = apply(&p.kex, &e.op, kex_name);
                    // Protocol extension pseudo-algorithms must always be advertised.
                    for ext in [
                        kex::EXTENSION_SUPPORT_AS_CLIENT,
                        kex::EXTENSION_OPENSSH_STRICT_KEX_AS_CLIENT,
                    ] {
                        if !list.contains(&ext) {
                            list.push(ext);
                        }
                    }
                    p.kex = Cow::Owned(list);
                }
                Key::Ciphers => p.cipher = Cow::Owned(apply(&p.cipher, &e.op, cipher_name)),
                Key::Macs => p.mac = Cow::Owned(apply(&p.mac, &e.op, mac_name)),
                // Client-side signature policy only; russh already falls back to
                // ssh-rsa when the server does not advertise rsa-sha2-*.
                Key::PubkeyAccepted => {}
            }
        }
        p
    }
}

fn apply<T: Clone + PartialEq>(
    current: &[T],
    op: &Op,
    parse: impl Fn(&str) -> Option<T>,
) -> Vec<T> {
    let parsed = |n: &[String]| n.iter().filter_map(|x| parse(x)).collect::<Vec<T>>();
    match op {
        Op::Append(n) => {
            let mut out = current.to_vec();
            for a in parsed(n) {
                if !out.contains(&a) {
                    out.push(a);
                }
            }
            out
        }
        Op::Remove(n) => {
            let rm = parsed(n);
            current
                .iter()
                .filter(|a| !rm.contains(a))
                .cloned()
                .collect()
        }
        Op::Prepend(n) => {
            let mut out = parsed(n);
            for a in current {
                if !out.contains(a) {
                    out.push(a.clone());
                }
            }
            out
        }
        Op::Replace(n) => parsed(n),
    }
}

/// Normalize user input (trim lines, drop blanks) for storage.
pub fn normalize(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openssh_style_lines() {
        let o = SshOptions::parse(
            "HostKeyAlgorithms=+ssh-rsa\n\n# comment\nPubkeyAcceptedKeyTypes=+ssh-rsa\nKexAlgorithms +diffie-hellman-group14-sha1",
        )
        .unwrap();
        assert!(!o.is_empty());
        let p = o.preferred();
        assert!(p.kex.contains(&kex::DH_G14_SHA1));
        assert!(p.kex.contains(&kex::CURVE25519), "append keeps defaults");
        assert!(p.key.contains(&Algorithm::Rsa { hash: None }));
    }

    #[test]
    fn replace_remove_prepend() {
        let p = SshOptions::parse("Ciphers=aes128-cbc,aes256-ctr")
            .unwrap()
            .preferred();
        assert_eq!(
            p.cipher.as_ref(),
            &[cipher::AES_128_CBC, cipher::AES_256_CTR]
        );

        let p = SshOptions::parse("Ciphers=-chacha20-poly1305@openssh.com")
            .unwrap()
            .preferred();
        assert!(!p.cipher.contains(&cipher::CHACHA20_POLY1305));

        let p = SshOptions::parse("MACs=^hmac-sha1").unwrap().preferred();
        assert_eq!(p.mac[0], mac::HMAC_SHA1);

        let p = SshOptions::parse("KexAlgorithms=diffie-hellman-group1-sha1")
            .unwrap()
            .preferred();
        assert_eq!(p.kex[0], kex::DH_G1_SHA1);
        assert!(p.kex.contains(&kex::EXTENSION_SUPPORT_AS_CLIENT));
    }

    #[test]
    fn rejects_unknown() {
        assert!(SshOptions::parse("ProxyJump=bastion").is_err());
        assert!(SshOptions::parse("Ciphers=+rot13").is_err());
        assert!(SshOptions::parse("HostKeyAlgorithms=").is_err());
        assert!(SshOptions::parse("garbage").is_err());
        assert!(SshOptions::parse("").unwrap().is_empty());
    }

    #[test]
    fn normalize_trims() {
        assert_eq!(normalize("  A=1 \n\n B=2\n"), "A=1\nB=2");
    }
}
