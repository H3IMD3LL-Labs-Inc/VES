use crate::config::{
    schema::root::ConfigProvider,
    lifecycle::{
        cfg_events::ConfigEvent,
        cfg_manager::ConfigManager,
    },
    persistence::{
        ConfigPersist,
        ConfigLoader,
        helpers::serialize::deserialize_root_config,
    },
};

/// Bootstrap the Core Agent from persisted configuration(LMDB) on first startup
/// or after a restart
/// If no persisted config exists, an empty vector is returned, and if loading/
/// deserialization fails, a "PersistFailed" ConfigEvent is returned, without a
/// ConfigVersion
pub fn bootstrap_from_persistence<P, L>(
    manager: &mut ConfigManager,
    loader: &L,
    persister: &P,
) -> Vec<ConfigEvent>
where
    P: ConfigPersist,
    L: ConfigLoader,
{
    match loader.load() {
        Ok(Some(bytes)) => {
            match deserialize_root_config(&bytes) {
                Ok(config) => {
                    manager.process_config(
                        ConfigProvider::StaticFile,
                        config,
                        &bytes,
                        persister,
                    )
                }
                Err(e) => vec![ConfigEvent::PersistFailed {
                    source: ConfigProvider::StaticFile,
                    version: None,
                    error: format!("Failed to deserialize config: {}", e),
                }],
            }
        }

        Ok(None) => {
            vec![]
        }

        Err(e) => vec![ConfigEvent::PersistFailed {
            source: ConfigProvider::StaticFile,
            version: None,
            error: e.to_string(),
        }],
    }
}