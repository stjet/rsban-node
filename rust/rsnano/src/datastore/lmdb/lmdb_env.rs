use super::{assert_success, mdb_env_create, MdbEnv};
use crate::{
    datastore::lmdb::{
        mdb_env_open, mdb_env_set_mapsize, mdb_env_set_maxdbs, MDB_MAPASYNC, MDB_NOMEMINIT,
        MDB_NOMETASYNC, MDB_NORDAHEAD, MDB_NOSUBDIR, MDB_NOSYNC, MDB_NOTLS, MDB_WRITEMAP,
    },
    running_within_valgrind, LmdbConfig, SyncStrategy,
};
use anyhow::Result;
use std::{
    fs::{create_dir_all, set_permissions, Permissions},
    os::unix::prelude::PermissionsExt,
    path::Path,
    ptr,
};

pub struct EnvOptions {
    pub config: LmdbConfig,
    pub use_no_mem_init: bool,
}

pub struct LmdbEnv {
    pub environment: *mut MdbEnv,
}

impl LmdbEnv {
    pub fn new(path: &Path, options: &EnvOptions) -> Result<Self> {
        let mut result = Self {
            environment: ptr::null_mut(),
        };
        result.init(path, options)?;
        Ok(result)
    }

    pub fn init(&mut self, path: &Path, options: &EnvOptions) -> Result<()> {
        let parent = path.parent().ok_or_else(|| anyhow!("no parent path"))?;
        create_dir_all(parent)?;
        let perms = Permissions::from_mode(0o700);
        let _ = set_permissions(parent, perms);
        assert_success(unsafe { mdb_env_create(&mut self.environment) });
        assert_success(unsafe {
            mdb_env_set_maxdbs(self.environment, options.config.max_databases)
        });
        let mut map_size = options.config.map_size;
        let max_valgrind_map_size = 16 * 1024 * 1024;
        if running_within_valgrind() && map_size > max_valgrind_map_size {
            // In order to run LMDB under Valgrind, the maximum map size must be smaller than half your available RAM
            map_size = max_valgrind_map_size;
        }
        assert_success(unsafe { mdb_env_set_mapsize(self.environment, map_size) });
        // It seems if there's ever more threads than mdb_env_set_maxreaders has read slots available, we get failures on transaction creation unless MDB_NOTLS is specified
        // This can happen if something like 256 io_threads are specified in the node config
        // MDB_NORDAHEAD will allow platforms that support it to load the DB in memory as needed.
        // MDB_NOMEMINIT prevents zeroing malloc'ed pages. Can provide improvement for non-sensitive data but may make memory checkers noisy (e.g valgrind).
        let mut environment_flags = MDB_NOSUBDIR | MDB_NOTLS | MDB_NORDAHEAD;
        if options.config.sync == SyncStrategy::NosyncSafe {
            environment_flags |= MDB_NOMETASYNC;
        } else if options.config.sync == SyncStrategy::NosyncUnsafe {
            environment_flags |= MDB_NOSYNC;
        } else if options.config.sync == SyncStrategy::NosyncUnsafeLargeMemory {
            environment_flags |= MDB_NOSYNC | MDB_WRITEMAP | MDB_MAPASYNC;
        }

        if !running_within_valgrind() && options.use_no_mem_init {
            environment_flags |= MDB_NOMEMINIT;
        }

        assert_success(unsafe { mdb_env_open(self.environment, path, environment_flags, 0o600) });
        Ok(())
    }

    pub fn close_env(&mut self) {
        self.environment = ptr::null_mut();
    }
}
