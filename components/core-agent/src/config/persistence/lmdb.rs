use crate::recovery::{
    environment::PersistenceEngine,
    database::AllowedDatabases,
    transaction::TxOutcome,
    txn_writer::TxnWriter,
};

use lmdb::{
    Transaction,
    Error as DbError
};
use super::traits::{ConfigPersist, ConfigLoader};
use super::types::LATEST_ROOT_CONFIG_KEY;

pub struct LmdbConfigStore<'a> {
    writer: TxnWriter<'a>,
    dbs: &'a AllowedDatabases,
}

impl<'a> LmdbConfigStore<'a> {
    pub fn new(
        engine: &'a PersistenceEngine,
        dbs: &'a AllowedDatabases
    ) -> Self {
        Self {
            writer: TxnWriter { env: engine },
            dbs,
        }
    }
}

impl<'a> ConfigPersist for LmdbConfigStore<'a> {
    fn persist(&self, config_bytes: &[u8]) -> Result<(), DbError> {
        self.writer.insert_item(
            self.dbs.persisted_config_db,
            &LATEST_ROOT_CONFIG_KEY,
            config_bytes,
        )
    }
}

impl<'a> ConfigLoader for LmdbConfigStore<'a> {
    fn load(&self) -> Result<Option<Vec<u8>>, DbError> {
        let mut result = None;

        // [TODO]: Replace with an actual read transaction from txn_reader.rs
        self.writer.write(|txn| {
            match txn.get(self.dbs.persisted_config_db, &LATEST_ROOT_CONFIG_KEY) {
                Ok(val) => result = Some(val.to_vec()),
                Err(lmdb::Error::NotFound) => result = None,
                Err(e) => return Err(e),
            }
            Ok(TxOutcome::Commit(()))
        })?;
        Ok(result)
    }
}
