//! Online storage which is not a children of any physical drive.

use anyhow::Result;
use byte_unit::Byte;
use byte_unit::UnitType;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path;

use crate::devices;

use super::{
    local_info::{self, LocalInfo},
    Storage, StorageExt, Storages,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct OnlineStorage {
    /// ID.
    name: String,
    /// Provider string (for the common information).
    pub provider: String,
    /// Capacity in bytes.
    capacity: u64,
    /// Device and local info pairs.
    local_infos: BTreeMap<String, LocalInfo>,
}

impl OnlineStorage {
    /// # Arguments
    /// - alias: for [`LocalInfo`]
    pub fn new(
        name: String,
        provider: String,
        capacity: u64,
        alias: String,
        path: path::PathBuf,
        device: &devices::Device,
    ) -> OnlineStorage {
        let local_info = local_info::LocalInfo::new(alias, path);
        OnlineStorage {
            name,
            provider,
            capacity,
            local_infos: BTreeMap::from([(device.name(), local_info)]),
        }
    }
}

impl StorageExt for OnlineStorage {
    fn name(&self) -> &String {
        &self.name
    }

    fn capacity(&self) -> Option<u64> {
        Some(self.capacity)
    }

    fn local_info(&self, device: &devices::Device) -> Option<&LocalInfo> {
        self.local_infos.get(&device.name())
    }

    fn mount_path(&self, device: &devices::Device) -> Option<std::path::PathBuf> {
        self.local_infos
            .get(&device.name())
            .map(|info| info.mount_path())
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

impl fmt::Display for OnlineStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "O {name:<10} {size:<10.2}    {provider:<10}",
            name = self.name(),
            size = Byte::from_u64(self.capacity).get_appropriate_unit(UnitType::Binary),
            provider = self.provider,
        )
    }
}
