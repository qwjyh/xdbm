//! CLI arguments

use crate::path;
use crate::PathBuf;
use clap::Args;
use clap::{Parser, Subcommand};
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

    /// Print config dir.
    Path {},

    /// Sync with git repo.
    Sync {
        /// Remote name to sync.
        remote_name: Option<String>,
    },

    /// Check config files validity.
    Check {},

    /// Generate completion script.
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
        #[arg(long)]
        src: Option<String>,
        /// Filter by backup destination storage name.
        #[arg(long)]
        dest: Option<String>,
        /// Filter by device where the backup is configured.
        #[arg(long)]
        device: Option<String>,
        /// Long display with more information.
        #[arg(short, long)]
        long: bool,
    },
    /// Record xdbm that the backup with the name has finished right now.
    Done {
        /// Name of the backup config.
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
