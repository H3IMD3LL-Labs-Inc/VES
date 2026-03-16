// A RootConfig configuration "version" represents a unique identifier for a
// RootConfig received from a Configuration Provider, this unique id is used
// to; persist the specific config + retrieve the specific config

use sha2::{Digest, Sha256};
use hex;
use std::fmt;
use std::str::FromStr;

const SHA256_HEX_LEN: usize = 64;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ConfigVersion {
    pub hash: String,
    pub timestamp: Option<u64>,
}

impl ConfigVersion {
    // Compute a new version from raw bytes of the canonical RootConfig
    pub fn from_bytes(bytes: &[u8], timestamp: Option<u64>) -> Self {
        let hash = Sha256::digest(bytes);
        Self {
            hash: hex::encode(hash),
            timestamp,
        }
    }

    // Compute a new version from a serialized RootConfig
    // Ensure canonical serialization (e.g., sorted maps, no random field ordering)
    pub fn from_root_config_bytes(config_bytes: &[u8]) -> Self {
        Self::from_bytes(config_bytes, None)
    }

    // Return a short 8-character prefix for logging
    pub fn short(&self) -> &str {
        &self.hash[..8]
    }
}

impl fmt::Debug for ConfigVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ts) = self.timestamp {
            write!(f, "ConfigVersion({}-{})", self.short(), ts)
        } else {
            write!(f, "ConfigVersion({})", self.short())
        }
    }
}

impl fmt::Display for ConfigVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.hash.fmt(f)
    }
}

impl FromStr for ConfigVersion {
    type Err = &'static str;

    fn from_str(s: &str) -> Result <Self, Self::Err> {
        if s.len() != SHA256_HEX_LEN {
            return Err("invalid SHA256 hex length");
        }

        if hex::decode(s).is_err() {
            return Err("invalid hex string");
        }

        Ok(Self {
            hash: s.to_string(),
            timestamp: None,
        })
    }
}
