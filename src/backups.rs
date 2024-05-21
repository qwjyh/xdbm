//! Backup config and its history.
//!

use core::panic;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::{
    devices::Device,
    storages::{StorageExt, Storages},
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
    /// `name()` of [`crate::storages::Storage`].
    /// Use `String` for serialization/deserialization.
    pub storage: String,
    /// Relative path to the `storage`.
    pub path: PathBuf,
}

impl BackupTarget {
    pub fn new(storage_name: String, relative_path: PathBuf) -> Self {
        BackupTarget {
            storage: storage_name,
            path: relative_path,
        }
    }

    pub fn path(&self, storages: &Storages, device: &Device) -> Result<PathBuf> {
        let parent = storages.get(&self.storage).unwrap();
        let parent_path = parent.mount_path(device)?;
        Ok(parent_path.join(self.path.clone()))
    }
}

/// Type of backup commands.
#[derive(Debug, Serialize, Deserialize)]
pub enum BackupCommand {
    ExternallyInvoked(ExternallyInvoked),
}

pub trait BackupCommandExt {
    fn name(&self) -> &String;

    fn note(&self) -> &String;
}

impl BackupCommandExt for BackupCommand {
    fn name(&self) -> &String {
        match self {
            BackupCommand::ExternallyInvoked(cmd) => cmd.name(),
        }
    }

    fn note(&self) -> &String {
        match self {
            BackupCommand::ExternallyInvoked(cmd) => cmd.note(),
        }
    }
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

impl BackupCommandExt for ExternallyInvoked {
    fn name(&self) -> &String {
        &self.name
    }

    fn note(&self) -> &String {
        &self.note
    }
}

/// Backup execution log.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupLog {
    pub datetime: DateTime<Local>,
    status: BackupResult,
    log: String,
}

impl BackupLog {
    pub fn new_with_current_time(status: BackupResult, log: String) -> BackupLog {
        let timestamp = Local::now();
        trace!("Generating timestamp: {:?}", timestamp);
        BackupLog {
            datetime: timestamp,
            status,
            log,
        }
    }
}

/// Result of backup.
#[derive(Debug, Serialize, Deserialize)]
pub enum BackupResult {
    Success,
    Failure,
}

impl BackupResult {
    pub fn from_exit_code(code: u64) -> Self {
        if code == 0 {
            Self::Success
        } else {
            Self::Failure
        }
    }
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

    pub fn device<'a>(&'a self, devices: &'a [Device]) -> Option<&Device> {
        devices.iter().find(|dev| dev.name() == self.device)
    }

    pub fn source(&self) -> &BackupTarget {
        &self.from
    }

    pub fn destination(&self) -> &BackupTarget {
        &self.to
    }

    pub fn command(&self) -> &BackupCommand {
        &self.command
    }

    pub fn add_log(&mut self, newlog: BackupLog) {
        self.logs.push(newlog)
    }

    /// Get the last backup.
    pub fn last_backup(&self) -> Option<&BackupLog> {
        self.logs.iter().max_by_key(|log| log.datetime)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Backups {
    pub list: BTreeMap<String, Backup>,
}

impl Backups {
    /// Empty [`Backups`].
    pub fn new() -> Backups {
        Backups {
            list: BTreeMap::new(),
        }
    }

    pub fn get(&self, name: &String) -> Option<&Backup> {
        self.list.get(name)
    }

    pub fn get_mut(&mut self, name: &String) -> Option<&mut Backup> {
        self.list.get_mut(name)
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
