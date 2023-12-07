//! Manipulate subdirectories of other storages, including directories.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, path};

use crate::devices;

use super::{local_info::LocalInfo, Storage, StorageExt};

/// Subdirectory of other [Storage]s.
#[derive(Serialize, Deserialize, Debug)]
pub struct Directory {
    name: String,
    parent: String,
    relative_path: path::PathBuf,
    notes: String,
    local_info: HashMap<String, LocalInfo>,
}

impl Directory {
    /// - `name`: id
    /// - `parent`: where the directory locates.
    /// - `relative_path`: path from root of the parent storage.
    /// - `notes`: supplimental notes.
    fn new(
        name: String,
        parent: String,
        relative_path: path::PathBuf,
        notes: String,
        local_info: HashMap<String, LocalInfo>,
    ) -> Directory {
        Directory {
            name,
            parent,
            relative_path,
            notes,
            local_info,
        }
    }

    pub fn try_from_device_path(
        name: String,
        path: path::PathBuf,
        notes: String,
        device: &devices::Device,
        storages: &HashMap<String, Storage>,
    ) -> Result<Directory> {
        let (parent_name, diff_path) = storages
            .iter()
            .filter(|(_k, v)| v.has_alias(&device))
            .filter_map(|(k, v)| {
                let diff = pathdiff::diff_paths(&path, v.mount_path(&device, &storages).unwrap())?;
                trace!("Pathdiff: {:?}", diff);
                if diff.components().any(|c| c == path::Component::ParentDir) {
                    None
                } else {
                    Some((k, diff))
                }
            })
            .min_by_key(|(_k, v)| {
                let diff_paths: Vec<path::Component<'_>> = v.components().collect();
                diff_paths.len()
            })
            .context(format!("Failed to compare diff of paths"))?;
        trace!("Selected parent: {}", parent_name);
        let local_info = LocalInfo::new("".to_string(), path);
        Ok(Directory::new(
            name,
            parent_name.clone(),
            diff_path,
            notes,
            HashMap::from([(device.name(), local_info)]),
        ))
    }

    pub fn update_note(self, notes: String) -> Directory {
        Directory::new(
            self.name,
            self.parent,
            self.relative_path,
            notes,
            self.local_info,
        )
    }

    /// Get parent `&Storage` of directory.
    fn parent<'a>(&'a self, storages: &'a HashMap<String, Storage>) -> Result<&Storage> {
        let parent = &self.parent;
        storages.get(parent).context(format!(
            "No parent {} exists for directory {}",
            parent,
            self.name()
        ))
    }

    /// Resolve mount path of directory with current device.
    fn mount_path(
        &self,
        device: &devices::Device,
        storages: &HashMap<String, Storage>,
    ) -> Result<path::PathBuf> {
        let parent = self.parent(&storages)?;
        let parent_mount_path = parent.mount_path(&device, &storages)?;
        Ok(parent_mount_path.join(self.relative_path.clone()))
    }
}

impl StorageExt for Directory {
    fn name(&self) -> &String {
        &self.name
    }

    fn local_info(&self, device: &devices::Device) -> Option<&LocalInfo> {
        self.local_info.get(&device.name())
    }

    fn mount_path(
        &self,
        device: &devices::Device,
        storages: &HashMap<String, Storage>,
    ) -> Result<path::PathBuf> {
        Ok(self.mount_path(device, storages)?)
    }

    /// This method doesn't use `mount_path`.
    fn bound_on_device(
        &mut self,
        alias: String,
        mount_point: path::PathBuf,
        device: &devices::Device,
    ) -> Result<()> {
        let new_local_info = LocalInfo::new(alias, mount_point);
        match self.local_info.insert(device.name(), new_local_info) {
            Some(v) => println!("Value updated. old val is: {:?}", v),
            None => println!("inserted new val"),
        };
        Ok(())
    }
}

impl fmt::Display for Directory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "S {name:<10} < {parent:<10}{relative_path:<10} : {notes}",
            name = self.name(),
            parent = self.parent,
            relative_path = self.relative_path.display(),
            notes = self.notes,
        )
    }
}
