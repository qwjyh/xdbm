//! Manipulates storages.

use crate::devices;
use crate::storages::directory::Directory;
use crate::storages::online_storage::OnlineStorage;
use crate::storages::physical_drive_partition::PhysicalDrivePartition;
use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ffi, fmt, fs, io, path, u64};

/// YAML file to store known storages..
pub const STORAGESFILE: &str = "storages.yml";

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum StorageType {
    Physical,
    SubDirectory,
    Online,
}

/// All storage types.
#[derive(Serialize, Deserialize, Debug)]
pub enum Storage {
    PhysicalStorage(PhysicalDrivePartition),
    SubDirectory(Directory),
    Online(OnlineStorage),
}

impl Storage {
    /// Full type name like "PhysicalStorage".
    pub fn typename(&self) -> &str {
        match self {
            Self::PhysicalStorage(_) => "PhysicalStorage",
            Self::SubDirectory(_) => "SubDirectory",
            Self::Online(_) => "OnlineStorage",
        }
    }

    /// Short type name with one letter like "P".
    pub fn shorttypename(&self) -> &str {
        match self {
            Self::PhysicalStorage(_) => "P",
            Self::SubDirectory(_) => "S",
            Self::Online(_) => "O",
        }
    }
}

impl StorageExt for Storage {
    fn name(&self) -> &String {
        match self {
            Self::PhysicalStorage(s) => s.name(),
            Self::SubDirectory(s) => s.name(),
            Self::Online(s) => s.name(),
        }
    }

    fn local_info(&self, device: &devices::Device) -> Option<&local_info::LocalInfo> {
        match self {
            Self::PhysicalStorage(s) => s.local_info(device),
            Self::SubDirectory(s) => s.local_info(device),
            Self::Online(s) => s.local_info(device),
        }
    }

    fn mount_path(
        &self,
        device: &devices::Device,
        storages: &HashMap<String, Storage>,
    ) -> Result<path::PathBuf> {
        match self {
            Self::PhysicalStorage(s) => s.mount_path(&device, &storages),
            Self::SubDirectory(s) => s.mount_path(&device, &storages),
            Self::Online(s) => s.mount_path(&device, &storages),
        }
    }

    fn bound_on_device(
        &mut self,
        alias: String,
        mount_point: path::PathBuf,
        device: &devices::Device,
    ) -> Result<()> {
        match self {
            Storage::PhysicalStorage(s) => s.bound_on_device(alias, mount_point, device),
            Storage::SubDirectory(s) => s.bound_on_device(alias, mount_point, device),
            Storage::Online(s) => s.bound_on_device(alias, mount_point, device),
        }
    }

    fn capacity(&self) -> Option<u64> {
        match self {
            Storage::PhysicalStorage(s) => s.capacity(),
            Storage::SubDirectory(s) => s.capacity(),
            Storage::Online(s) => s.capacity(),
        }
    }
}

impl fmt::Display for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhysicalStorage(s) => s.fmt(f),
            Self::SubDirectory(s) => s.fmt(f),
            Self::Online(s) => s.fmt(f),
        }
    }
}

/// Trait to manipulate all `Storage`s (Enums).
pub trait StorageExt {
    fn name(&self) -> &String;

    /// Capacity in bytes.
    /// Since [Directory] has no capacity information, it has `None`.
    fn capacity(&self) -> Option<u64>;

    /// Return `true` if `self` has local info on the `device`.
    /// Used to check if the storage is bound on the `device`.
    fn has_alias(&self, device: &devices::Device) -> bool {
        self.local_info(device).is_some()
    }

    /// Get local info of `device`.
    fn local_info(&self, device: &devices::Device) -> Option<&local_info::LocalInfo>;

    /// Get mount path of `self` on `device`.
    /// `storages` is a `HashMap` with key of storage name and value of the storage.
    fn mount_path(
        &self,
        device: &devices::Device,
        storages: &HashMap<String, Storage>,
    ) -> Result<path::PathBuf>;

    /// Add local info of `device` to `self`.
    fn bound_on_device(
        &mut self,
        alias: String,
        mount_point: path::PathBuf,
        device: &devices::Device,
    ) -> Result<()>;
}

pub mod directory;
pub mod local_info;
pub mod online_storage;
pub mod physical_drive_partition;

/// Get `HashMap<String, Storage>` from devices.yml([devices::DEVICESFILE]).
/// If [devices::DEVICESFILE] isn't found, return empty vec.
pub fn get_storages(config_dir: &path::Path) -> Result<HashMap<String, Storage>> {
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
        let yaml: HashMap<String, Storage> =
            serde_yaml::from_reader(reader).context("Failed to read devices.yml")?;
        Ok(yaml)
    } else {
        trace!("No {} found", STORAGESFILE);
        Ok(HashMap::new())
    }
}

/// Write `storages` to yaml file in `config_dir`.
pub fn write_storages(config_dir: &path::Path, storages: HashMap<String, Storage>) -> Result<()> {
    let f = fs::File::create(config_dir.join(STORAGESFILE))?;
    let writer = io::BufWriter::new(f);
    serde_yaml::to_writer(writer, &storages).map_err(|e| anyhow!(e))
}
