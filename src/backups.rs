use core::panic;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::{
    devices::Device,
    storages::{self, Storage},
};

/// Directory to store backup configs for each devices.
pub const BACKUPSDIR: &str = "backups";

/// File to store backups for the `device`.
/// Relative path from the config directory.
pub fn backups_file(device: &Device) -> PathBuf {
    PathBuf::from(BACKUPSDIR).join(format!("{}.yml", device.name()))
}

/// Targets for backup source or destination.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupTarget {
    /// `name()` of [`Storage`].
    /// Use `String` for serialization/deserialization.
    storage: String,
    /// Relative path to the `storage`.
    path: PathBuf,
}

impl BackupTarget {
    pub fn new(storage_name: String, relative_path: PathBuf) -> Self {
        BackupTarget {
            storage: storage_name,
            path: relative_path,
        }
    }
}

/// Type of backup commands.
#[derive(Debug, Serialize, Deserialize)]
pub enum BackupCommand {
    ExternallyInvoked(ExternallyInvoked),
}

/// Backup commands which is not invoked from xdbm itself.
/// Call xdbm externally to record backup datetime and status.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExternallyInvoked {
    name: String,
    pub note: String,
}

impl ExternallyInvoked {
    pub fn new(name: String, note: String) -> Self {
        ExternallyInvoked { name, note }
    }
}

/// Backup execution log.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupLog {
    datetime: DateTime<Local>,
    status: BackupResult,
    log: String,
}

/// Result of backup.
#[derive(Debug, Serialize, Deserialize)]
pub enum BackupResult {
    Success,
    Failure,
}

/// Backup source, destination, command and logs.
#[derive(Debug, Serialize, Deserialize)]
pub struct Backup {
    /// must be unique
    name: String,
    /// name of [`crate::Device`]
    device: String,
    from: BackupTarget,
    to: BackupTarget,
    command: BackupCommand,
    logs: Vec<BackupLog>,
}

impl Backup {
    /// With empty logs.
    pub fn new(
        name: String,
        device_name: String,
        from: BackupTarget,
        to: BackupTarget,
        command: BackupCommand,
    ) -> Self {
        Backup {
            name,
            device: device_name,
            from,
            to,
            command,
            logs: Vec::new(),
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Backups {
    pub list: HashMap<String, Backup>,
}

impl Backups {
    /// Empty [`Backups`].
    pub fn new() -> Backups {
        Backups {
            list: HashMap::new(),
        }
    }

    pub fn get(&self, name: &String) -> Option<&Backup> {
        self.list.get(name)
    }

    /// Add new [`Backup`].
    /// New `backup` must has new unique name.
    pub fn add(&mut self, backup: Backup) -> Result<()> {
        if self.list.keys().any(|name| name == &backup.name) {
            return Err(anyhow::anyhow!(format!(
                "Backup with name {} already exists",
                backup.name
            )));
        }
        match self.list.insert(backup.name.clone(), backup) {
            Some(v) => {
                error!("Inserted backup with existing name: {}", v.name);
                panic!("unexpected behavior (unreachable)")
            }
            None => Ok(()),
        }
    }

    pub fn read(config_dir: &Path, device: &Device) -> Result<Backups> {
        let backups_file = config_dir.join(backups_file(device));
        if !backups_file.exists() {
            return Err(anyhow!("Couldn't find backups file: {:?}", backups_file));
        }
        trace!("Reading {}", backups_file.display());
        let f = fs::File::open(backups_file)?;
        let reader = io::BufReader::new(f);
        let yaml: Backups =
            serde_yaml::from_reader(reader).context("Failed to parse backups file")?;
        Ok(yaml)
    }

    pub fn write(self, config_dir: &Path, device: &Device) -> Result<()> {
        let f = fs::File::create(config_dir.join(backups_file(device)))
            .context("Failed to open backups file")?;
        let writer = io::BufWriter::new(f);
        serde_yaml::to_writer(writer, &self).context(format!(
            "Failed writing to {}",
            config_dir.join(backups_file(device)).display()
        ))
    }
}
