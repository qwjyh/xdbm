//! # Main types
//! * [Device]: represents PC. module [devices]
//! * [Storage]: all storages. module [storages]
//!     * [physical_drive_partition::PhysicalDrivePartition]: partition on a physical disk. module [storages::physical_drive_partition]
//!     * [directory::Directory]: sub-directory of other storages. module [storages::directory]
//!     * [online_storage::OnlineStorage]: online storage like Google Drive. module [storages::online_storage]
//! * [storages::local_info::LocalInfo]: stores [Device] specific common data for [Storage]s.
//!

#[macro_use]
extern crate log;

extern crate dirs;

use anyhow::{anyhow, Context, Result};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use git2::{Commit, Oid, Repository};
use inquire::{min_length, Confirm, CustomType, Select};
use inquire::{validator::Validation, Text};
use serde_yaml;
use std::collections::HashMap;
use std::path::Path;
use std::path::{self, PathBuf};

use crate::cmd_args::{Cli, Commands, StorageCommands};
use crate::cmd_storage;
use crate::storages::online_storage;
use crate::storages::{
    directory, get_storages, local_info, physical_drive_partition, write_storages, Storage,
    StorageExt, StorageType, STORAGESFILE,
};
use devices::{Device, DEVICESFILE, *};

mod cmd_args;
mod cmd_init;
mod cmd_storage;
mod devices;
mod inquire_filepath_completer;
mod storages;

struct BackupLog {}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();
    trace!("Start logging...");

    let mut config_dir: std::path::PathBuf =
        dirs::config_local_dir().context("Failed to get config dir.")?;
    config_dir.push("xdbm");
    trace!("Config dir: {:?}", config_dir);

    match cli.command {
        Commands::Init { repo_url } => cmd_init::cmd_init(repo_url, &config_dir)?,
        Commands::Storage(storage) => {
            let repo = Repository::open(&config_dir).context(
                "Repository doesn't exist. Please run init to initialize the repository.",
            )?;
            trace!("repo state: {:?}", repo.state());
            match storage.command {
                StorageCommands::Add { storage_type, path } => {
                    cmd_storage::cmd_storage_add(storage_type, path, repo, &config_dir)?
                }
                StorageCommands::List {} => cmd_storage::cmd_storage_list(&config_dir)?,
                StorageCommands::Bind {
                    storage: storage_name,
                    alias: new_alias,
                    path: mount_point,
                } => cmd_storage::cmd_storage_bind(
                    storage_name,
                    new_alias,
                    mount_point,
                    repo,
                    &config_dir,
                )?,
            }
        }
        Commands::Path {} => {
            println!("{}", &config_dir.display());
        }
        Commands::Sync {} => {
            unimplemented!("Sync is not implemented")
        }
        Commands::Check {} => {
            println!("Config dir: {}", &config_dir.display());
            let _storages =
                storages::get_storages(&config_dir).context("Failed to parse storages file.");
            todo!()
        }
    }
    full_status(&Repository::open(&config_dir)?)?;
    Ok(())
}

/// Set device name interactively.
fn set_device_name() -> Result<Device> {
    let validator = |input: &str| {
        if input.chars().count() == 0 {
            Ok(Validation::Invalid("Need at least 1 character.".into()))
        } else {
            Ok(Validation::Valid)
        }
    };

    let device_name = Text::new("Provide name for this device:")
        .with_validator(validator)
        .prompt();

    let device_name = match device_name {
        Ok(device_name) => {
            println!("device name: {}", device_name);
            device_name
        }
        Err(err) => {
            println!("Error {}", err);
            return Err(anyhow!(err));
        }
    };

    let device = Device::new(device_name);
    trace!("Device information: {:?}", device);
    trace!("Serialized: \n{}", serde_yaml::to_string(&device).unwrap());

    return Ok(device);
}

fn ask_unique_name(storages: &HashMap<String, Storage>, target: String) -> Result<String> {
    let mut disk_name = String::new();
    loop {
        disk_name = Text::new(format!("Name for {}:", target).as_str()).prompt()?;
        if storages.iter().all(|(k, v)| k != &disk_name) {
            break;
        }
        println!("The name {} is already used.", disk_name);
    }
    Ok(disk_name)
}

fn find_last_commit(repo: &Repository) -> Result<Option<Commit>, git2::Error> {
    if repo.is_empty()? {
        Ok(None)
    } else {
        let commit = repo
            .head()?
            .resolve()?
            .peel(git2::ObjectType::Commit)?
            .into_commit()
            .map_err(|_| git2::Error::from_str("Couldn't find commit"))?;
        Ok(Some(commit))
    }
}

/// Add file and commit
fn add_and_commit(repo: &Repository, path: &Path, message: &str) -> Result<Oid, git2::Error> {
    trace!("repo state: {:?}", repo.state());
    full_status(repo).unwrap();
    let mut index = repo.index()?;
    index.add_path(path)?;
    full_status(repo).unwrap();
    index.write()?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;
    let config = git2::Config::open_default()?;
    let signature = git2::Signature::now(
        config.get_entry("user.name")?.value().unwrap(),
        config.get_entry("user.email")?.value().unwrap(),
    )?;
    trace!("git signature: {}", signature);
    let parent_commit = find_last_commit(&repo)?;
    let result = match parent_commit {
        Some(parent_commit) => repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        ),
        None => repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[]),
    };
    trace!("repo state: {:?}", repo.state());
    full_status(repo).unwrap();
    result
}

/// Print git repo status as trace
fn full_status(repo: &Repository) -> Result<()> {
    trace!("status: ");
    for status in repo.statuses(None)?.iter() {
        let path = status.path().unwrap_or("");
        let st = status.status();
        trace!("  {}: {:?}", path, st);
    }
    Ok(())
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}
