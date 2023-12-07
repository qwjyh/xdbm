//! Device specific common data for storages.

use serde::{Deserialize, Serialize};
use std::path;

/// Store local (device-specific) information
///
/// - `alias`: name in device
/// - `mount_path`: mount path on the device
#[derive(Serialize, Deserialize, Debug)]
pub struct LocalInfo {
    alias: String,
    mount_path: path::PathBuf,
}

impl LocalInfo {
    pub fn new(alias: String, mount_path: path::PathBuf) -> LocalInfo {
        LocalInfo { alias, mount_path }
    }

    pub fn alias(&self) -> String {
        self.alias.clone() // ?
    }

    pub fn mount_path(&self) -> path::PathBuf {
        self.mount_path.clone()
    }
}
