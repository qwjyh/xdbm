//! Manipulate partition of physical drive (both removable and unremovable).

use crate::devices::Device;
use crate::storages::StorageExt;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::RandomState, HashMap};
use sysinfo::DiskExt;

/// Partitoin of physical (on-premises) drive.
#[derive(Serialize, Deserialize, Debug)]
pub struct PhysicalDrivePartition {
    name: String,
    kind: String,
    capacity: u64,
    fs: String,
    is_removable: bool,
    system_names: HashMap<String, String, RandomState>,
}

impl PhysicalDrivePartition {
    /// Try to get Physical drive info from sysinfo.
    pub fn try_from_sysinfo_disk(
        disk: &sysinfo::Disk,
        name: String,
        device: Device,
    ) -> Result<PhysicalDrivePartition> {
        let alias = disk
            .name()
            .to_str()
            .context("Failed to convert storage name to valid str.")?
            .to_string();
        let fs = disk.file_system();
        trace!("fs: {:?}", fs);
        let fs = std::str::from_utf8(fs)?;
        Ok(PhysicalDrivePartition {
            name: name,
            kind: format!("{:?}", disk.kind()),
            capacity: disk.total_space(),
            fs: fs.to_string(),
            is_removable: disk.is_removable(),
            system_names: HashMap::from([(device.name(), alias)]),
        })
    }

    fn add_alias(
        self,
        disk: sysinfo::Disk,
        device: Device,
    ) -> Result<PhysicalDrivePartition, String> {
        let alias = match disk.name().to_str() {
            Some(s) => s.to_string(),
            None => return Err("Failed to convert storage name to valid str.".to_string()),
        };
        let mut aliases = self.system_names;
        let _ = match aliases.insert(device.name(), alias) {
            Some(v) => v,
            None => return Err("Failed to insert alias".to_string()),
        };
        Ok(PhysicalDrivePartition {
            name: self.name,
            kind: self.kind,
            capacity: self.capacity,
            fs: self.fs,
            is_removable: self.is_removable,
            system_names: aliases,
        })
    }
}

impl StorageExt for PhysicalDrivePartition {
    fn name(&self) -> &String {
        &self.name
    }
}
