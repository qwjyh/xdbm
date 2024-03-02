//! Device specific common data for storages.

use serde::{Deserialize, Serialize};
use std::path::{self, PathBuf};

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

#[test]
fn localinfo() {
    let localinfo = LocalInfo::new("alias".to_string(), PathBuf::from("/mnt/sample"));
    assert_eq!(localinfo.alias(), "alias".to_string());
    assert_eq!(localinfo.mount_path(), PathBuf::from("/mnt/sample"));
}
