use lmdb::{
    Environment,
    EnvironmentFlags,
};
use std::sync::Arc;
use std::path::Path;

pub struct PersistenceEngine {
    pub engine: Arc<Environment>,
}

impl PersistenceEngine {
    pub fn new_env(path: &Path ) -> anyhow::Result<Self> {
        let env = Environment::new()
            .set_max_dbs(10)
            .set_max_readers(120)
            .set_map_size(1024 * 1024 * 1024 * 1024)
            .open(path)?;

        Ok(Self {
            engine: Arc::new(env),
        })
    }

    pub fn check_env(path: &Path) -> bool {
        let mut env_builder = Environment::new();
        env_builder.set_max_dbs(10);
        env_builder.set_max_readers(120);
        env_builder.set_map_size(1024 * 1024 * 1024 * 1024);
        env_builder.set_flags(EnvironmentFlags::READ_ONLY);

        match env_builder.open(path) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    // [TODO]: Add a method that uses Environment::stat() to give a view of
    //         the environment's Stat struct containing; psize, depth, branch_pages,
    //         leaf_pages, overflow_pages and entries. This will allow; capacity
    //         planning, PersistenceEngine monitoring, debugging performance
}
