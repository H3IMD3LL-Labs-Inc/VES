// Local crates
use crate::recovery::environment::PersistenceEngine;

// External crates
use lmdb::{
    Database,
    RoTransaction,
    RwTransaction,
    Transaction,
    Error as DbError,
};

pub enum TxOutcome<T> {
    Commit(T),
    Abort(T),
}

pub fn with_ro<T, F>(
    env: &PersistenceEngine,
    f: F,
) -> Result<T, DbError>
where
    F: FnOnce(&RoTransaction) -> Result<T, DbError>,
{
    let txn = env.engine.begin_ro_txn()?;
    f(&txn)
}

pub fn read_only_cursor<'txn>(
    txn: &'txn RoTransaction,
    db: Database,
) -> Result<lmdb::RoCursor<'txn>, DbError> {
    txn.open_ro_cursor(db)
}

/// Consider a single writer thread where all Core Agent stages send
/// write requests into a channel, and the thread handles all writes
/// sequentially inside a single write transaction to avoid blocking
/// multiple threads, and each stage continues working asynchronously
/// while the writer thread commits sequentially
///
/// [Stage A] \
/// [Stage B]  ---> [Writer Thread] ---> LMDB commit sequence
/// [Stage C] /
///
pub fn with_rw<T, F>(
    env: &PersistenceEngine,
    f: F,
) -> Result<T, DbError>
where
    F: FnOnce(&mut RwTransaction) -> Result<TxOutcome<T>, DbError>,
{
    let mut txn = env.engine.begin_rw_txn()?;
    match f(&mut txn)? {
        TxOutcome::Commit(val) => {
            txn.commit()?;
            Ok(val)
        }
        TxOutcome::Abort(val) => {
            txn.abort();
            Ok(val)
        }
    }
}
