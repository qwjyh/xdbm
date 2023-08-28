//! Manipulates storages.

use clap::ValueEnum;
use physical_drive_partition::PhysicalDrivePartition;
use serde::{Deserialize, Serialize};

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
    pub fn name(&self) -> &String {
        match self {
            Self::PhysicalStorage(s) => s.name(),
        }
    }
}

/// Trait to manipulate all `Storage`s (Enums).
pub trait StorageExt {
    fn name(&self) -> &String;
}

pub mod physical_drive_partition;
