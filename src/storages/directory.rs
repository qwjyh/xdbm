//! Manipulate subdirectories of other storages, including directories.

use serde::{Deserialize, Serialize};
use std::{path, fmt::{self, write}, rc::Rc};

use super::{Storage, StorageExt};

/// Subdirectory of other [Storage]s.
#[derive(Serialize, Deserialize, Debug)]
pub struct Directory {
    name: String,
    parent: Box<Storage>,
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
        parent: Rc<Storage>, // todo implement serialize
        relative_path: path::PathBuf,
        notes: String,
    ) -> Directory {
        Directory {name, parent, relative_path, notes}
    }
}

impl StorageExt for Directory {
    fn name(&self) -> &String {
        &self.name
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