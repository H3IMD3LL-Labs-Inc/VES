// Local crates
use crate::recovery::{
    environment::PersistenceEngine,
    transaction::{
        with_ro,
        read_only_cursor,
    },
};

// External crates
use lmdb::{
    Transaction,
    Database,
    Cursor,
    Error as DbError,
};
use lmdb_sys as ffi;

enum Direction {
    Forward,
    Backward,
}

struct TraverseConfig<'a> {
    start: Option<&'a [u8]>,
    end: Option<&'a [u8]>,
    prefix: Option<&'a [u8]>,
    direction: Direction,
    limit: Option<usize>,
}

pub struct TxnReader<'a> {
    env: &'a PersistenceEngine,
}

impl<'a> TxnReader<'a> {
    fn traversal_internal<F>(
        &self,
        db: Database,
        config: TraverseConfig,
        mut f: F,
    ) -> Result<usize, DbError>
    where
        F: FnMut(&[u8], &[u8]),
    {
        with_ro(self.env, |txn| {
            let mut cursor = read_only_cursor(txn, db)?;

            let mut processed = 0;

            match config.direction {
                Direction::Forward => {
                    let iter = match config.start {
                        Some(start) => cursor.iter_from(start),
                        None => cursor.iter_start(),
                    };

                    for item in iter {
                        let (key, value) = item;

                        if let Some(end) = config.end {
                            if key >= end {
                                break;
                            }
                        }

                        if let Some(prefix) = config.prefix {
                            if !key.starts_with(prefix) {
                                break;
                            }
                        }

                        f(key, value);
                        processed += 1;

                        if let Some(limit) = config.limit {
                            if processed >= limit {
                                break;
                            }
                        }
                    }
                }

                Direction::Backward => {
                    if let Some(start) = config.start {
                        cursor.get(Some(start), None, ffi::MDB_SET_RANGE)?;
                    } else {
                        // [NOTE]: Incase of first key not included in TraverseConfig
                        //         backwards traversal will be for the entire database
                        cursor.get(None, None, ffi::MDB_LAST)?;
                    }

                    loop {
                        match cursor.get(None, None, ffi::MDB_PREV) {
                            Ok((Some(key), value)) => {
                                if let Some(end) = config.end {
                                    if key < end {
                                        break;
                                    }
                                }

                                if let Some(prefix) = config.prefix {
                                    if !key.starts_with(prefix) {
                                        break;
                                    }
                                }

                                f(key, value);
                                processed += 1;

                                if let Some(limit) = config.limit {
                                    if processed >= limit {
                                        break;
                                    }
                                }
                            }

                            Err(lmdb::Error::NotFound) => break,
                            Err(e) => return Err(e),
                            _ => break,
                        }
                    }
                }
            }

            Ok(processed)
        })
    }
}
