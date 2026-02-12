//! CLI arguments

use crate::PathBuf;
use crate::backups;
use crate::devices;
use crate::path;
use crate::storages;
use clap::Args;
use clap::{Parser, Subcommand};
use clap_complete::ArgValueCandidates;
use clap_complete::CompletionCandidate;
use clap_verbosity_flag::Verbosity;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,

    /// Customized config dir.
    #[arg(short, long)]
    pub(crate) config_dir: Option<PathBuf>,

    #[command(flatten)]
    pub(crate) verbose: Verbosity,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Initialize for this device.
    Init {
        /// Name for this device
        device_name: String,
        /// Url for existing repository. Empty if init for the first time.
        #[arg(short, long)]
        repo_url: Option<String>, // url?
        /// Whether to use ssh-agent
        #[arg(long)]
        use_sshagent: bool,
        /// Manually specify ssh key
        #[arg(long)]
        ssh_key: Option<PathBuf>,
    },

    /// Manage storages.
    Storage(StorageArgs),

    /// Manage backups.
    #[command(subcommand)]
    Backup(BackupSubCommands),

    /// Print status for the given path.
    Status {
        /// Target path. Default is the current directory.
        path: Option<PathBuf>,
        /// Show storage which the path belongs to.
        #[arg(short, long)]
        storage: bool,
        /// Show backup config covering the path.
        #[arg(short, long)]
        backup: bool,
    },

    /// Print config dir.
    Path {},

    /// Sync with git repo.
    Sync {
        /// Remote name to sync.
        remote_name: Option<String>,
        /// Use custom git implementation.
        #[arg(short, long)]
        use_libgit2: bool,
        /// Whether to use ssh-agent
        #[arg(long)]
        use_sshagent: bool,
        /// Manually specify ssh key
        #[arg(long)]
        ssh_key: Option<PathBuf>,
    },

    /// Check config files validity.
    Check {},

    /// [DEPRECATED] Generate completion script.
    ///
    /// Use xdbm native completion instead;
    /// Source `COMPLETE=<SHELL> xdbm`.
    Completion { shell: clap_complete::Shell },
}

#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct StorageArgs {
    #[command(subcommand)]
    pub(crate) command: StorageCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum StorageCommands {
    /// Add new storage.
    Add(StorageAddArgs),
    /// List all storages.
    List {
        /// Show note on the storages.
        #[arg(short, long)]
        long: bool,
    },
    /// Make `storage` available for the current device.
    /// For physical disk, the name is taken from system info automatically.
    Bind {
        /// Name of the storage.
        #[arg(add = ArgValueCandidates::new(storage_name_completer))]
        storage: String,
        /// Device specific alias for the storage.
        #[arg(short, long)]
        alias: String,
        /// Mount point on this device.
        #[arg(short, long)]
        path: path::PathBuf,
    },
    // /// Remove storage from the storage list
    // Remove {
    //     storage: String,
    // }
}

#[derive(Args, Debug)]
pub(crate) struct StorageAddArgs {
    #[command(subcommand)]
    pub(crate) command: StorageAddCommands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum StorageAddCommands {
    /// Physical drive partition.
    Physical {
        /// Unique name for the storage.
        name: String,
        /// Path where the storage is mounted on this device.
        /// leave blank to fetch system info automatically.
        path: Option<PathBuf>,
    },
    /// Sub directory of other storages.
    Directory {
        /// Unique name for the storage.
        name: String,
        /// Path where the storage is mounted on this device.
        path: PathBuf,
        /// Additional info. Empty by default.
        #[arg(short, long, default_value = "")]
        notes: String,
        /// Device specific alias for the storage.
        #[arg(short, long)]
        alias: String,
    },
    /// Online storage.
    Online {
        /// Unique name for the storage.
        name: String,
        /// Path where the storage is mounted on this device.
        path: PathBuf,
        /// Provider name (for the common information).
        #[arg(short, long)]
        provider: String,
        /// Capacity in bytes.
        #[arg(short, long)]
        capacity: u64,
        /// Device specific alias for the storage.
        #[arg(short, long)]
        alias: String,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum BackupSubCommands {
    /// Add new backup config.
    Add {
        name: String,
        /// Source of the data backuped.
        #[arg(short, long)]
        src: PathBuf,
        /// Destination of the backuped data.
        #[arg(short, long)]
        dest: PathBuf,
        #[command(subcommand)]
        cmd: BackupAddCommands,
    },
    /// Print configured backups.
    /// Filter by src/dest storage or device.
    List {
        /// Filter by backup source storage name.
        #[arg(long, add = ArgValueCandidates::new(storage_name_completer))]
        src: Option<String>,
        /// Filter by backup destination storage name.
        #[arg(long, add = ArgValueCandidates::new(storage_name_completer))]
        dest: Option<String>,
        /// Filter by device where the backup is configured.
        #[arg(long, add = ArgValueCandidates::new(device_name_completer))]
        device: Option<String>,
        /// Long display with more information.
        #[arg(short, long)]
        long: bool,
    },
    /// Record xdbm that the backup with the name has finished right now.
    Done {
        /// Name of the backup config.
        #[arg(add = ArgValueCandidates::new(backup_name_completer_local))]
        name: String,
        /// Result of the backup
        exit_status: u64,
        /// Optional log or note about the backup execution.
        #[arg(short, long)]
        log: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum BackupAddCommands {
    /// Invoke logging via cli of xdbm. The simplest one.
    External {
        name: String,
        #[arg(default_value = "")]
        note: String,
    },
}

fn storage_name_completer() -> Vec<CompletionCandidate> {
    let mut completions = vec![];
    // TODO: support custom config dir with env var
    let config_dir = match crate::default_config_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to get config dir: {e}");
            return completions;
        }
    };
    let storages = match storages::Storages::read(&config_dir) {
        Ok(storages) => storages,
        Err(e) => {
            eprintln!("{e}");
            return completions;
        }
    };

    completions.extend(storages.list.keys().map(CompletionCandidate::new));
    completions
}

fn device_name_completer() -> Vec<CompletionCandidate> {
    let config_dir = match crate::default_config_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to get config dir: {e}");
            return vec![];
        }
    };
    let devices = match devices::get_devices(&config_dir) {
        Ok(devices) => devices,
        Err(e) => {
            eprintln!("{e}");
            return vec![];
        }
    };
    devices
        .into_iter()
        .map(|device| CompletionCandidate::new(device.name()))
        .collect()
}

fn backup_name_completer_local() -> Vec<CompletionCandidate> {
    let config_dir = match crate::default_config_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to get config dir: {e}");
            return vec![];
        }
    };
    let device = match devices::get_device(&config_dir) {
        Ok(device) => device,
        Err(e) => {
            eprintln!("failed to get device: {e}");
            return vec![];
        }
    };
    let backups = match backups::Backups::read(&config_dir, &device) {
        Ok(backups) => backups,
        Err(e) => {
            eprintln!("{e}");
            return vec![];
        }
    };
    backups.list.keys().map(CompletionCandidate::new).collect()
}
