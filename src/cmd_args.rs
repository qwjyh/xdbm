//! CLI arguments

use crate::StorageType;
use crate::path;
use crate::PathBuf;
use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;

#[derive(Parser)]
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

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Initialize for this device.
    /// Provide `repo_url` to use existing repository, otherwise this device will be configured as the
    /// first device.
    Init {
        repo_url: Option<String>, // url?
    },

    /// Manage storages.
    Storage(StorageArgs),

    /// Print config dir.
    Path {},

    /// Sync with git repo.
    Sync {},

    /// Check config files.
    Check {},
}

#[derive(clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct StorageArgs {
    #[command(subcommand)]
    pub(crate) command: StorageCommands,
}

#[derive(Subcommand)]
pub(crate) enum StorageCommands {
    /// Add new storage.
    Add {
        #[arg(value_enum)]
        storage_type: StorageType,

        // TODO: set this require and select matching disk for physical
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
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
}
