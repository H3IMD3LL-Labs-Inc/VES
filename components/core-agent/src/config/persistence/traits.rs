use lmdb::Error as DbError;

pub trait ConfigPersist {
    fn persist(&self, config_bytes: &[u8]) -> Result<(), DbError>;
}

pub trait ConfigLoader {
    fn load(&self) -> Result<Option<Vec<u8>>, DbError>;
}
