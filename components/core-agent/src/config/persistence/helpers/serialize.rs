use crate::config::schema::root::RootConfig;

use toml;

pub fn serialize_root_config(config: &RootConfig) -> Vec<u8> {
    toml::to_string(config)
        .expect("Failed to serialize RootConfig to TOML")
        .into_bytes()
}

pub fn deserialize_root_config(bytes: &[u8]) -> Result<RootConfig, String> {
    toml::from_slice(bytes).map_err(|err| format!(
        "Failed to deserialize RootConfig from bytes: {}", err
    ))
}