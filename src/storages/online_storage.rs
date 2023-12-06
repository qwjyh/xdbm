//! Online storage which is not a children of any physical drive.

use anyhow::Context;
use byte_unit::Byte;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path;

use crate::devices;

use super::local_info::LocalInfo;
use super::StorageExt;

#[derive(Serialize, Deserialize, Debug)]
pub struct OnlineStorage {
    name: String,
    provider: String,
    capacity: u8,
    local_info: HashMap<String, LocalInfo>,
}

impl OnlineStorage {
    fn new(name: String, provider: String, capacity: u8, path: path::PathBuf, device: &devices::Device) -> OnlineStorage {
        todo!()
    }
}

impl StorageExt for OnlineStorage {
    fn name(&self) -> &String {
        &self.name
    }

    fn has_alias(&self, device: &devices::Device) -> bool {
        self.local_info.get(&device.name()).is_some()
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
