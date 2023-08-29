//! Manipulates storages.

use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use physical_drive_partition::PhysicalDrivePartition;
use serde::{Deserialize, Serialize};
use std::{ffi, fmt, fs, path::Path, io};

/// YAML file to store known storages..
pub const STORAGESFILE: &str = "storages.yml";

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum StorageType {
    Physical,
    // Online,
}

/// All storage types.
#[derive(Serialize, Deserialize, Debug)]
pub enum Storage {
    PhysicalStorage(PhysicalDrivePartition),
    // /// Online storage provided by others.
    // OnlineStorage {
    //     name: String,
    //     provider: String,
    //     capacity: u8,
    // },
}

impl Storage {
    pub fn add_alias(&mut self, disk: &sysinfo::Disk, config_dir: &std::path::PathBuf) -> anyhow::Result<()> {
        match self {
            Self::PhysicalStorage(s) => s.add_alias(disk, config_dir),
        }
    }
}

impl StorageExt for Storage {
    fn name(&self) -> &String {
        match self {
            Self::PhysicalStorage(s) => s.name(),
        }
    }

}

impl fmt::Display for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhysicalStorage(s) => s.fmt(f),
        }
    }
}

/// Trait to manipulate all `Storage`s (Enums).
pub trait StorageExt {
    fn name(&self) -> &String;
}

pub mod physical_drive_partition;

/// Get `Vec<Storage>` from devices.yml([DEVICESFILE]).
/// If [DEVICESFILE] isn't found, return empty vec.
pub fn get_storages(config_dir: &Path) -> Result<Vec<Storage>> {
    if let Some(storages_file) = fs::read_dir(&config_dir)?
        .filter(|f| {
            f.as_ref().map_or_else(
                |_e| false,
                |f| {
                    let storagesfile: ffi::OsString = STORAGESFILE.into();
                    f.path().file_name() == Some(&storagesfile)
                },
            )
        })
        .next()
    {
        trace!("{} found: {:?}", STORAGESFILE, storages_file);
        let f = fs::File::open(config_dir.join(STORAGESFILE))?;
        let reader = io::BufReader::new(f);
        let yaml: Vec<Storage> =
            serde_yaml::from_reader(reader).context("Failed to read devices.yml")?;
        Ok(yaml)
    } else {
        trace!("No {} found", STORAGESFILE);
        Ok(vec![])
    }
}

/// Write `storages` to yaml file in `config_dir`.
pub fn write_storages(config_dir: &Path, storages: Vec<Storage>) -> Result<()> {
    let f = fs::File::create(config_dir.join(STORAGESFILE))?;
    let writer = io::BufWriter::new(f);
    serde_yaml::to_writer(writer, &storages).map_err(|e| anyhow!(e))
}