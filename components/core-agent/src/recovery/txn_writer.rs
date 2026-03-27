use crate::recovery::{
    environment::PersistenceEngine,
    transaction::{
        TxOutcome,
        with_rw
    },
};

use lmdb::{
    RwTransaction,
    Database,
    WriteFlags,
    Error as DbError,
};

pub struct TxnWriter<'a> {
    pub env: &'a PersistenceEngine,
}

impl<'a> TxnWriter<'a> {
    pub fn write<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&mut RwTransaction) -> Result<TxOutcome<T>, DbError>
    {
        with_rw(self.env, f)
    }

    pub fn insert_item<K, V>(
        &self,
        db: Database,
        key: K,
        value: V,
    ) -> Result<(), DbError>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        self.write(|txn| {
            txn.put(db, &key, &value, WriteFlags::empty())?;
            Ok(TxOutcome::Commit(()))
        })
    }

    pub fn delete_item<K>(
        &self,
        db: Database,
        key: K,
    ) -> Result<(), DbError>
    where
        K: AsRef<[u8]>,
    {
        self.write(|txn| {
            txn.del(db, &key, None)?;
            Ok(TxOutcome::Commit(()))
        })
    }
}
