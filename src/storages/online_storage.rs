//! Online storage which is not a children of any physical drive.

use anyhow::{Context, Result};
use byte_unit::Byte;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path;

use crate::devices;

use super::local_info;
use super::local_info::LocalInfo;
use super::StorageExt;

#[derive(Serialize, Deserialize, Debug)]
pub struct OnlineStorage {
    name: String,
    provider: String,
    capacity: u64,
    local_info: HashMap<String, LocalInfo>,
}

impl OnlineStorage {
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
            local_info: HashMap::from([(device.name(), local_info)]),
        }
    }
}

impl StorageExt for OnlineStorage {
    fn name(&self) -> &String {
        &self.name
    }

    fn local_info(&self, device: &devices::Device) -> Option<&LocalInfo> {
        self.local_info.get(&device.name())
    }

    fn mount_path(
        &self,
        device: &devices::Device,
        _storages: &HashMap<String, super::Storage>,
    ) -> anyhow::Result<std::path::PathBuf> {
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

impl fmt::Display for OnlineStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "O {name:<10} {size}    {provider:<10}",
            name = self.name(),
            size = Byte::from_bytes(self.capacity.into()).get_appropriate_unit(true),
            provider = self.provider,
        )
    }
}
