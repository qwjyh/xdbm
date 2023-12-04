use serde::{Deserialize, Serialize};
use std::path;

#[derive(Serialize, Deserialize, Debug)]
pub struct LocalInfo {
    alias: String,
    mount_path: path::PathBuf,
}

impl LocalInfo {
    pub fn new(alias: String, mount_path: path::PathBuf) -> LocalInfo {
        LocalInfo { alias, mount_path }
    }

    pub fn mount_path(&self) -> &path::PathBuf {
        &self.mount_path
    }
}
