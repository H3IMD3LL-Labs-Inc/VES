// Local crates
use crate::recovery::environment::PersistenceEngine;

// External crates
use lmdb::{
    Database,
    DatabaseFlags,
    Error as DbError,
};

pub struct AllowedDatabases {
    pub engine: PersistenceEngine,
    pub watcher_db: Database,
    pub tailer_db: Database,
}

impl AllowedDatabases {
    pub fn new(env: PersistenceEngine) -> Result<Self, DbError> {
        let watcher_db = env
            .engine
            .create_db(Some("watcher_db"), DatabaseFlags::empty())?;

        let tailer_db = env
            .engine
            .create_db(Some("tailer_db"), DatabaseFlags::empty())?;

        Ok(Self {
            engine: env,
            watcher_db,
            tailer_db,
        })
    }
}
