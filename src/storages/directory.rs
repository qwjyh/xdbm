//! Manipulate subdirectories of other storages, including directories.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{collections::BTreeMap, fmt, path};

use crate::devices;
use crate::util;

use super::{local_info::LocalInfo, Storage, StorageExt, Storages};

/// Subdirectory of other [Storage]s.
#[derive(Serialize, Deserialize, Debug)]
pub struct Directory {
    /// ID.
    name: String,
    /// ID of parent storage.
    parent: String,
    /// Relative path to the parent storage.
    relative_path: Vec<String>,
    pub notes: String,
    /// [`devices::Device`] name and localinfo pairs.
    local_infos: BTreeMap<String, LocalInfo>,
}

impl Directory {
    /// - `name`: id
    /// - `parent`: where the directory locates.
    /// - `relative_path`: path from root of the parent storage.
    /// - `notes`: supplemental notes.
    fn new(
        name: String,
        parent: String,
        relative_path: path::PathBuf,
        notes: String,
        local_infos: BTreeMap<String, LocalInfo>,
    ) -> Result<Directory> {
        let relative_path = relative_path
            .components()
            .map(|c| c.as_os_str().to_str().map(|s| s.to_owned()))
            .collect::<Option<Vec<_>>>()
            .context("Path contains non-utf8 character")?;
        Ok(Directory {
            name,
            parent,
            relative_path,
            notes,
            local_infos,
        })
    }

    pub fn try_from_device_path(
        name: String,
        path: path::PathBuf,
        notes: String,
        alias: String,
        device: &devices::Device,
        storages: &Storages,
    ) -> Result<Directory> {
        let (parent, diff_path) = util::min_parent_storage(&path, storages, device)
            .context("Failed to compare diff of paths")?;
        trace!("Selected parent: {}", parent.name());
        let local_info = LocalInfo::new(alias, path);
        Directory::new(
            name,
            parent.name().to_string(),
            diff_path,
            notes,
            BTreeMap::from([(device.name(), local_info)]),
        )
    }

    pub fn update_note(self, notes: String) -> Directory {
        Directory {
            name: self.name,
            parent: self.parent,
            relative_path: self.relative_path,
            notes,
            local_infos: self.local_infos,
        }
    }

    /// Resolve mount path of directory with current device.
    fn mount_path(&self, device: &devices::Device, storages: &Storages) -> Result<path::PathBuf> {
        let parent_mount_path = self
            .parent(storages)
            .context("Can't find parent storage")?
            .mount_path(device)
            .context("Can't find mount path")?;
        Ok(parent_mount_path.join(self.relative_path.clone().iter().collect::<PathBuf>()))
    }
}

impl StorageExt for Directory {
    fn name(&self) -> &String {
        &self.name
    }

    fn capacity(&self) -> Option<u64> {
        None
    }

    fn local_info(&self, device: &devices::Device) -> Option<&LocalInfo> {
        self.local_infos.get(&device.name())
    }

    fn mount_path(&self, device: &devices::Device) -> Option<std::path::PathBuf> {
        self.local_infos
            .get(&device.name())
            .map(|info| info.mount_path())
    }

    /// This method doesn't use `mount_path`.
    fn bound_on_device(
        &mut self,
        alias: String,
        mount_point: path::PathBuf,
        device: &devices::Device,
    ) -> Result<()> {
        let new_local_info = LocalInfo::new(alias, mount_point);
        match self.local_infos.insert(device.name(), new_local_info) {
            Some(v) => println!("Value updated. old val is: {:?}", v),
            None => println!("inserted new val"),
        };
        Ok(())
    }

    // Get parent `&Storage` of directory.
    fn parent<'a>(&'a self, storages: &'a Storages) -> Option<&'a Storage> {
        storages.get(&self.parent)
    }
}

impl fmt::Display for Directory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "S {name:<10} < {parent:<10}{relative_path:<10} : {notes}",
            name = self.name(),
            parent = self.parent,
            relative_path = self.relative_path.iter().collect::<PathBuf>().display(),
            notes = self.notes,
        )
    }
}

#[cfg(test)]
mod test {
    use std::{collections::BTreeMap, path::PathBuf};

    use crate::{
        devices::Device,
        storages::{
            self, local_info::LocalInfo, physical_drive_partition::PhysicalDrivePartition, Storage,
            StorageExt, Storages,
        },
    };

    use super::Directory;

    #[test]
    fn name() {
        let local_info_phys =
            LocalInfo::new("phys_alias".to_string(), PathBuf::from("/mnt/sample"));
        let local_info_dir =
            LocalInfo::new("dir_alias".to_string(), PathBuf::from("/mnt/sample/subdir"));
        let device = Device::new("test_device".to_string());
        let mut local_infos = BTreeMap::new();
        local_infos.insert(device.name(), local_info_dir);
        let physical = PhysicalDrivePartition::new(
            "parent".to_string(),
            "SSD".to_string(),
            1_000_000_000,
            "btrfs".to_string(),
            false,
            local_info_phys,
            &device,
        );
        let directory = Directory::new(
            "test_name".to_owned(),
            "parent".to_string(),
            "subdir".into(),
            "some note".to_string(),
            local_infos,
        )
        .unwrap();
        let mut storages = Storages::new();
        storages.add(storages::Storage::Physical(physical)).unwrap();
        storages.add(Storage::SubDirectory(directory)).unwrap();
        // assert_eq!(directory.name(), "test_name");
        assert_eq!(
            storages.get(&"test_name".to_string()).unwrap().name(),
            "test_name"
        );
        assert_eq!(
            storages
                .get(&"test_name".to_string())
                .unwrap()
                .mount_path(&device)
                .unwrap(),
            PathBuf::from("/mnt/sample/subdir")
        );
    }
}
