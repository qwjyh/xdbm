//! Manipulate partition of physical drive (both removable and unremovable).

use crate::{devices::Device, get_device};
use crate::storages::{Storage, StorageExt};
use anyhow::{anyhow, Context, Result};
use byte_unit::Byte;
use inquire::Text;
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::RandomState, HashMap},
    fmt,
};
use sysinfo::{Disk, DiskExt, SystemExt};

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
            Some(v) => println!("Value updated. old val is: {}", v),
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


/// Interactively select physical storage from available disks in sysinfo.
pub fn select_physical_storage(
    device: Device,
    storages: &Vec<Storage>,
) -> Result<PhysicalDrivePartition> {
    trace!("select_physical_storage");
    // get disk info fron sysinfo
    let mut sys_disks = sysinfo::System::new_all();
    sys_disks.refresh_disks();
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
        if storages.iter().all(|s| s.name() != &disk_name) {
            break;
        }
        println!("The name {} is already used.", disk_name);
    }
    trace!("selected name: {}", disk_name);
    PhysicalDrivePartition::try_from_sysinfo_disk(&disk, disk_name, device)
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
