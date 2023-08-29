//! Manipulate partition of physical drive (both removable and unremovable).

use crate::{devices::Device, get_device};
use crate::storages::StorageExt;
use anyhow::{anyhow, Context, Result};
use byte_unit::Byte;
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::RandomState, HashMap},
    fmt,
};
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

    pub fn add_alias(
        &mut self,
        disk: &sysinfo::Disk,
        config_dir: &std::path::PathBuf
    ) -> Result<()> {
        let device = get_device(&config_dir)?;
        let alias = match disk.name().to_str() {
            Some(s) => s.to_string(),
            None => return Err(anyhow!("Failed to convert storage name to valid str.")),
        };
        let aliases = &mut self.system_names;
        trace!("aliases: {:?}", aliases);
        match aliases.insert(device.name(), alias) {
            Some(v) => trace!("old val is: {}", v),
            None => trace!("inserted new val"),
        };
        trace!("aliases: {:?}", aliases);
        // self.system_names = aliases;
        Ok(())
    }
}

impl StorageExt for PhysicalDrivePartition {
    fn name(&self) -> &String {
        &self.name
    }

}

impl fmt::Display for PhysicalDrivePartition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let removable_indicator = if self.is_removable { "+" } else { "-" };
        write!(
            f,
            "{name:<10} {size}  {removable:<1} {kind:<6} {fs:<5}",
            name = self.name(),
            size = Byte::from_bytes(self.capacity.into()).get_appropriate_unit(true),
            removable = removable_indicator,
            kind = self.kind,
            fs = self.fs,
            // path = self. TODO: display path or contain it in struct
        )
    }
}
