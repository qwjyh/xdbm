//! Manipulate partition of physical drive (both removable and unremovable).

use crate::devices;
use crate::devices::Device;
use crate::storages::{Storage, StorageExt, Storages};
use anyhow::{anyhow, Context, Result};
use byte_unit::Byte;
use serde::{Deserialize, Serialize};
use std::path::{self, Path};
use std::{collections::BTreeMap, fmt};
use sysinfo::{Disk, Disks};

use super::local_info::{self, LocalInfo};

/// Partitoin of physical (on-premises) drive.
#[derive(Serialize, Deserialize, Debug)]
pub struct PhysicalDrivePartition {
    name: String,
    kind: String,
    capacity: u64,
    fs: String,
    is_removable: bool,
    // system_names: BTreeMap<String, String>,
    local_infos: BTreeMap<String, LocalInfo>,
}

impl PhysicalDrivePartition {
    pub fn new(
        name: String,
        kind: String,
        capacity: u64,
        fs: String,
        is_removable: bool,
        local_info: LocalInfo,
        device: &Device,
    ) -> PhysicalDrivePartition {
        PhysicalDrivePartition {
            name,
            kind,
            capacity,
            fs,
            is_removable,
            local_infos: BTreeMap::from([(device.name(), local_info)]),
        }
    }

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
        let fs: String = fs
            .to_str()
            .context("Failed to convert file_system osstr")?
            .to_owned();
        let local_info = LocalInfo::new(alias, disk.mount_point().to_path_buf());
        Ok(PhysicalDrivePartition {
            name,
            kind: format!("{:?}", disk.kind()),
            capacity: disk.total_space(),
            fs: fs.to_string(),
            is_removable: disk.is_removable(),
            // system_names: BTreeMap::from([(device.name(), alias)]),
            local_infos: BTreeMap::from([(device.name(), local_info)]),
        })
    }

    pub fn bind_device(&mut self, disk: &sysinfo::Disk, config_dir: &Path) -> Result<()> {
        let device = devices::get_device(config_dir)?;
        let alias = match disk.name().to_str() {
            Some(s) => s.to_string(),
            None => return Err(anyhow!("Failed to convert storage name to valid str.")),
        };
        let new_local_info = LocalInfo::new(alias, disk.mount_point().to_path_buf());
        let aliases = &mut self.local_infos;
        trace!("aliases: {:?}", aliases);
        match aliases.insert(device.name(), new_local_info) {
            Some(v) => println!("Value updated. old val is: {:?}", v),
            None => println!("inserted new val"),
        };
        trace!("aliases: {:?}", aliases);
        Ok(())
    }

    pub fn is_removable(&self) -> bool {
        self.is_removable
    }

    pub fn kind(&self) -> &String {
        &self.kind
    }
}

impl StorageExt for PhysicalDrivePartition {
    fn name(&self) -> &String {
        &self.name
    }

    fn capacity(&self) -> Option<u64> {
        Some(self.capacity)
    }

    fn local_info(&self, device: &devices::Device) -> Option<&local_info::LocalInfo> {
        self.local_infos.get(&device.name())
    }

    fn mount_path(&self, device: &devices::Device) -> Result<path::PathBuf> {
        Ok(self
            .local_infos
            .get(&device.name())
            .context(format!("LocalInfo for storage: {} not found", &self.name()))?
            .mount_path())
    }

    fn bound_on_device(
        &mut self,
        alias: String,
        mount_point: path::PathBuf,
        device: &devices::Device,
    ) -> Result<()> {
        match self
            .local_infos
            .insert(device.name(), LocalInfo::new(alias, mount_point))
        {
            Some(old) => info!("Value replaced. Old value: {:?}", old),
            None => info!("New value inserted."),
        };
        Ok(())
    }

    fn parent(&self, _storages: &Storages) -> Option<&Storage> {
        None
    }
}

impl fmt::Display for PhysicalDrivePartition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let removable_indicator = if self.is_removable { "+" } else { "-" };
        write!(
            f,
            "P {name:<10} {size:<10}  {removable:<1} {kind:<6} {fs:<5}",
            name = self.name(),
            size = Byte::from_bytes(self.capacity.into()).get_appropriate_unit(true),
            removable = removable_indicator,
            kind = self.kind,
            fs = self.fs,
            // path = self. TODO: display path or contain it in struct
        )
    }
}

/// Interactively select physical storage from available disks in sysinfo.
pub fn select_physical_storage(
    disk_name: String,
    device: Device,
) -> Result<PhysicalDrivePartition> {
    trace!("select_physical_storage");
    // get disk info from sysinfo
    let sys_disks = Disks::new_with_refreshed_list();
    trace!("Available disks");
    for disk in &sys_disks {
        trace!("{:?}", disk)
    }
    let disk = select_sysinfo_disk(&sys_disks)?;
    let storage = PhysicalDrivePartition::try_from_sysinfo_disk(disk, disk_name, device)?;
    Ok(storage)
}

fn select_sysinfo_disk(disks: &sysinfo::Disks) -> Result<&Disk> {
    let available_disks = disks
        .iter()
        .enumerate()
        .map(|(i, disk)| {
            let name = disk.name().to_str().unwrap_or("");
            let fs: &str = disk.file_system().to_str().unwrap_or("unknown");
            let kind = format!("{:?}", disk.kind());
            let mount_path = disk.mount_point();
            let total_space = byte_unit::Byte::from_bytes(disk.total_space().into())
                .get_appropriate_unit(true)
                .to_string();
            format!(
                "{}: {} {} ({}, {}) {}",
                i,
                name,
                total_space,
                fs,
                kind,
                mount_path.display()
            )
        })
        .collect();
    // select from list
    let disk = inquire::Select::new("Select drive:", available_disks).prompt()?;
    let disk_num: usize = disk.split(':').next().unwrap().parse().unwrap();
    trace!("disk_num: {}", disk_num);
    let disk = disks
        .get(disk_num)
        .context("no disk matched with selected one.")?;
    trace!("selected disk: {:?}", disk);
    Ok(disk)
}

#[cfg(test)]
mod test {
    use crate::{
        devices::Device,
        storages::{local_info::LocalInfo, StorageExt},
    };
    use std::path::PathBuf;

    use super::PhysicalDrivePartition;

    #[test]
    fn test_new() {
        let localinfo = LocalInfo::new("alias".to_string(), PathBuf::from("/mnt/sample"));
        let storage = PhysicalDrivePartition::new(
            "name".to_string(),
            "SSD".to_string(),
            100,
            "ext_4".to_string(),
            true,
            localinfo,
            &Device::new("test_device".to_string()),
        );
        assert_eq!(storage.name(), "name");
        assert_eq!(storage.capacity(), Some(100));
    }
}
