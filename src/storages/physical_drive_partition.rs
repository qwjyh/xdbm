//! Manipulate partition of physical drive (both removable and unremovable).

use crate::devices;
use crate::storages::{Storage, StorageExt};
use crate::{devices::Device, get_device};
use anyhow::{anyhow, Context, Result};
use byte_unit::Byte;
use inquire::Text;
use serde::{Deserialize, Serialize};
use std::path;
use std::{
    collections::{hash_map::RandomState, HashMap},
    fmt,
};
use sysinfo::{Disk, DiskExt, SystemExt};

use super::local_info::{self, LocalInfo};

/// Partitoin of physical (on-premises) drive.
#[derive(Serialize, Deserialize, Debug)]
pub struct PhysicalDrivePartition {
    name: String,
    kind: String,
    capacity: u64,
    fs: String,
    is_removable: bool,
    // system_names: HashMap<String, String>,
    local_info: HashMap<String, LocalInfo>,
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
            local_info: HashMap::from([(device.name(), local_info)]),
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
        let fs = std::str::from_utf8(fs)?;
        let local_info = LocalInfo::new(alias, disk.mount_point().to_path_buf().canonicalize()?);
        Ok(PhysicalDrivePartition {
            name: name,
            kind: format!("{:?}", disk.kind()),
            capacity: disk.total_space(),
            fs: fs.to_string(),
            is_removable: disk.is_removable(),
            // system_names: HashMap::from([(device.name(), alias)]),
            local_info: HashMap::from([(device.name(), local_info)]),
        })
    }

    pub fn bind_device(
        &mut self,
        disk: &sysinfo::Disk,
        config_dir: &std::path::PathBuf,
    ) -> Result<()> {
        let device = get_device(&config_dir)?;
        let alias = match disk.name().to_str() {
            Some(s) => s.to_string(),
            None => return Err(anyhow!("Failed to convert storage name to valid str.")),
        };
        let new_local_info = LocalInfo::new(alias, disk.mount_point().to_path_buf());
        let aliases = &mut self.local_info;
        trace!("aliases: {:?}", aliases);
        match aliases.insert(device.name(), new_local_info) {
            Some(v) => println!("Value updated. old val is: {:?}", v),
            None => println!("inserted new val"),
        };
        trace!("aliases: {:?}", aliases);
        Ok(())
    }
}

impl StorageExt for PhysicalDrivePartition {
    fn name(&self) -> &String {
        &self.name
    }

    fn local_info(&self, device: &devices::Device) -> Option<&local_info::LocalInfo> {
        self.local_info.get(&device.name())
    }

    fn mount_path(
        &self,
        device: &devices::Device,
        _: &HashMap<String, Storage>,
    ) -> Result<path::PathBuf> {
        Ok(self
            .local_info
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
            .local_info
            .insert(device.name(), LocalInfo::new(alias, mount_point))
        {
            Some(old) => info!("Value replaced. Old value: {:?}", old),
            None => info!("New value inserted."),
        };
        Ok(())
    }
}

impl fmt::Display for PhysicalDrivePartition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let removable_indicator = if self.is_removable { "+" } else { "-" };
        write!(
            f,
            "P {name:<10} {size}  {removable:<1} {kind:<6} {fs:<5}",
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
    device: Device,
    storages: &HashMap<String, Storage>,
) -> Result<(String, PhysicalDrivePartition)> {
    trace!("select_physical_storage");
    // get disk info fron sysinfo
    let sys_disks =
        sysinfo::System::new_with_specifics(sysinfo::RefreshKind::new().with_disks_list());
    trace!("refresh");
    // sys_disks.refresh_disks_list();
    // sys_disks.refresh_disks();
    trace!("Available disks");
    for disk in sys_disks.disks() {
        trace!("{:?}", disk)
    }
    let disk = select_sysinfo_disk(&sys_disks)?;
    // name the disk
    let mut disk_name = String::new();
    trace!("{}", disk_name);
    loop {
        disk_name = Text::new("Name for the disk:").prompt()?;
        if storages.iter().all(|(k, v)| k != &disk_name) {
            break;
        }
        println!("The name {} is already used.", disk_name);
    }
    trace!("selected name: {}", disk_name);
    let storage = PhysicalDrivePartition::try_from_sysinfo_disk(&disk, disk_name.clone(), device)?;
    Ok((disk_name, storage))
}

pub fn select_sysinfo_disk(sysinfo: &sysinfo::System) -> Result<&Disk> {
    let available_disks = sysinfo
        .disks()
        .iter()
        .enumerate()
        .map(|(i, disk)| {
            let name = match disk.name().to_str() {
                Some(s) => s,
                None => "",
            };
            let fs: &str = std::str::from_utf8(disk.file_system()).unwrap_or("unknown");
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
    let disk = sysinfo
        .disks()
        .iter()
        .nth(disk_num)
        .context("no disk matched with selected one.")?;
    trace!("selected disk: {:?}", disk);
    Ok(disk)
}
