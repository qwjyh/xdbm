//! Manipulate subdirectories of other storages, including directories.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, write},
    path,
    rc::Rc,
};

use crate::devices;

use super::{Storage, StorageExt};

/// Subdirectory of other [Storage]s.
#[derive(Serialize, Deserialize, Debug)]
pub struct Directory {
    name: String,
    parent: String,
    relative_path: path::PathBuf,
    notes: String,
}

impl Directory {
    /// - `name`: id
    /// - `parent`: where the directory locates.
    /// - `relative_path`: path from root of the parent storage.
    /// - `notes`: supplimental notes.
    pub fn new(
        name: String,
        parent: String, // todo implement serialize
        relative_path: path::PathBuf,
        notes: String,
    ) -> Directory {
        Directory {
            name,
            parent,
            relative_path,
            notes,
        }
    }

    /// Get parent `&Storage` of directory.
    fn parent<'a>(&'a self, storages: &'a HashMap<String, Storage>) -> Result<&Storage> {
        let parent = &self.parent;
        storages.get(&self.parent.clone()).context(format!(
            "No parent {} exists for directory {}",
            parent,
            self.name()
        ))
    }

    // /// Resolve mount path of directory with current device.
    // fn mount_path(
    //     &self,
    //     &device: &devices::Device,
    //     &storages: &HashMap<String, Storage>,
    // ) -> Result<path::PathBuf> {
    //     let parent = self.parent(&storages)?;
    //     let parent_mount_path = parent.mount_path(&device, &storages)?;
    //     Ok(parent_mount_path.join(self.relative_path.clone()))
    // }
}

impl StorageExt for Directory {
    fn name(&self) -> &String {
        &self.name
    }

    // fn mount_path(&self, &device: &devices::Device, &storages: &HashMap<String, Storage>) -> Result<&path::PathBuf> {
    //     Ok(&self.mount_path(&device, &storages)?)
    // }
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
