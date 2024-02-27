/// CLI arguments definition

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;

use crate::storages::StorageType;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[command(flatten)]
    pub verbose: Verbosity,
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
    List {},
    /// Add new device-specific name to existing storage.
    /// For physical disk, the name is taken from system info automatically.
    Bind { storage: String },
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}
