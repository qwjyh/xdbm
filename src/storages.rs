//! Manipulates storages.

use crate::devices;
use crate::storages::{
    directory::Directory, online_storage::OnlineStorage,
    physical_drive_partition::PhysicalDrivePartition,
};
use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use console::{style, Style, StyledObject};
use core::panic;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, fs, io, path};

/// YAML file to store known storages..
pub const STORAGESFILE: &str = "storages.yml";

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum StorageType {
    /// Physical storage
    P,
    /// Sub directory
    S,
    /// Online storage
    O,
}

/// All storage types.
#[derive(Serialize, Deserialize, Debug)]
pub enum Storage {
    Physical(PhysicalDrivePartition),
    SubDirectory(Directory),
    Online(OnlineStorage),
}

impl Storage {
    /// Full type name like "PhysicalStorage".
    pub fn typename(&self) -> &str {
        match self {
            Self::Physical(_) => "PhysicalStorage",
            Self::SubDirectory(_) => "SubDirectory",
            Self::Online(_) => "OnlineStorage",
        }
    }

    /// Short type name with one letter like "P".
    pub fn shorttypename(&self) -> &str {
        match self {
            Self::Physical(_) => "P",
            Self::SubDirectory(_) => "S",
            Self::Online(_) => "O",
        }
    }

    pub fn typestyle(&self) -> Style {
        match self {
            Storage::Physical(_) => Style::new().cyan(),
            Storage::SubDirectory(_) => Style::new().yellow(),
            Storage::Online(_) => Style::new().green(),
        }
    }
}

impl StorageExt for Storage {
    fn name(&self) -> &String {
        match self {
            Self::Physical(s) => s.name(),
            Self::SubDirectory(s) => s.name(),
            Self::Online(s) => s.name(),
        }
    }

    fn local_info(&self, device: &devices::Device) -> Option<&local_info::LocalInfo> {
        match self {
            Self::Physical(s) => s.local_info(device),
            Self::SubDirectory(s) => s.local_info(device),
            Self::Online(s) => s.local_info(device),
        }
    }

    fn mount_path(&self, device: &devices::Device) -> Option<path::PathBuf> {
        match self {
            Self::Physical(s) => s.mount_path(device),
            Self::SubDirectory(s) => s.mount_path(device),
            Self::Online(s) => s.mount_path(device),
        }
    }

    fn bound_on_device(
        &mut self,
        alias: String,
        mount_point: path::PathBuf,
        device: &devices::Device,
    ) -> Result<()> {
        match self {
            Storage::Physical(s) => s.bound_on_device(alias, mount_point, device),
            Storage::SubDirectory(s) => s.bound_on_device(alias, mount_point, device),
            Storage::Online(s) => s.bound_on_device(alias, mount_point, device),
        }
    }

    fn capacity(&self) -> Option<u64> {
        match self {
            Storage::Physical(s) => s.capacity(),
            Storage::SubDirectory(s) => s.capacity(),
            Storage::Online(s) => s.capacity(),
        }
    }

    fn parent<'a>(&'a self, storages: &'a Storages) -> Option<&'a Storage> {
        match self {
            Storage::Physical(s) => s.parent(storages),
            Storage::SubDirectory(s) => s.parent(storages),
            Storage::Online(s) => s.parent(storages),
        }
    }
}

impl fmt::Display for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Physical(s) => s.fmt(f),
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
    /// Return [`None`] if the storage([`self`]) is not configured for the `device`.
    fn mount_path(&self, device: &devices::Device) -> Option<path::PathBuf>;

    /// Add local info of `device` to `self`.
    fn bound_on_device(
        &mut self,
        alias: String,
        mount_point: path::PathBuf,
        device: &devices::Device,
    ) -> Result<()>;

    /// Get parent
    fn parent<'a>(&'a self, storages: &'a Storages) -> Option<&Storage>;
}

pub mod directory;
pub mod local_info;
pub mod online_storage;
pub mod physical_drive_partition;

#[derive(Debug, Serialize, Deserialize)]
pub struct Storages {
    pub list: BTreeMap<String, Storage>,
}

impl Storages {
    /// Construct empty [`Storages`]
    pub fn new() -> Storages {
        Storages {
            list: BTreeMap::new(),
        }
    }

    /// Get [`Storage`] with `name`.
    pub fn get(&self, name: &String) -> Option<&Storage> {
        self.list.get(name)
    }

    /// Add new [`Storage`] to [`Storages`]
    /// New `storage` must has new unique name.
    pub fn add(&mut self, storage: Storage) -> Result<()> {
        if self.list.keys().any(|name| name == storage.name()) {
            return Err(anyhow!(format!(
                "Storage name {} already used",
                storage.name()
            )));
        }
        match self.list.insert(storage.name().to_string(), storage) {
            Some(v) => {
                error!("Inserted storage with existing name: {}", v);
                panic!("unexpected behavior")
            }
            None => Ok(()),
        }
    }

    /// Remove `storage` from [`Storages`].
    /// Returns `Result` of removed [`Storage`].
    pub fn remove(mut self, storage: Storage) -> Result<Option<Storage>> {
        // dependency check
        if self.list.iter().any(|(_k, v)| {
            v.parent(&self)
                .is_some_and(|parent| parent.name() == storage.name())
        }) {
            return Err(anyhow!(
                "Dependency error: storage {} has some children",
                storage.name()
            ));
        }
        Ok(self.list.remove(storage.name()))
    }

    /// Load [`Storages`] from data in `config_dir`.
    pub fn read(config_dir: &path::Path) -> Result<Self> {
        let storages_file = config_dir.join(STORAGESFILE);
        if !storages_file.exists() {
            warn!("No storages file found.");
            return Err(anyhow!("Couln't find {}", STORAGESFILE));
        }
        trace!("Reading {:?}", storages_file);
        let f = fs::File::open(storages_file)?;
        let reader = io::BufReader::new(f);
        let yaml: Storages =
            serde_yaml::from_reader(reader).context("Failed to parse storages.yml")?;
        Ok(yaml)
    }

    pub fn write(self, config_dir: &path::Path) -> Result<()> {
        let f = fs::File::create(config_dir.join(STORAGESFILE))
            .context("Failed to open storages file")?;
        let writer = io::BufWriter::new(f);
        serde_yaml::to_writer(writer, &self)
            .context(format!("Failed to writing to {:?}", STORAGESFILE))
    }
}
