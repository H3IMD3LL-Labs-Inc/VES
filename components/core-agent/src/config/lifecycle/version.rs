use crate::config::{
    schema::root::RootConfig,
    persistence::helpers::serialize::serialize_root_config,
};

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
    pub fn from_bytes(bytes: &[u8], timestamp: Option<u64>) -> Self {
        let hash = Sha256::digest(bytes);
        Self {
            hash: hex::encode(hash),
            timestamp,
        }
    }

    pub fn from_root_config(config: &RootConfig) -> Self {
        let bytes = serialize_root_config(config);
        Self::from_bytes(&bytes, None)
    }

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
