use std::path::PathBuf;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::storages::Storage;

/// Targets for backup source or destination.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupTarget {
    storage: Storage,
    path: PathBuf,
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
