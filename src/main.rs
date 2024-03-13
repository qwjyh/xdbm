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
use clap::{CommandFactory, Parser};
use git2::{Commit, Oid, Repository};
use std::path::Path;
use std::path::{self, PathBuf};
use storages::Storages;

use crate::cmd_args::{BackupSubCommands, Cli, Commands, StorageCommands};
use crate::storages::{
    directory, local_info, online_storage, physical_drive_partition, Storage, StorageExt,
    StorageType, STORAGESFILE,
};
use devices::{Device, DEVICESFILE, *};

mod backups;
mod cmd_args;
mod cmd_backup;
mod cmd_init;
mod cmd_storage;
mod cmd_sync;
mod devices;
mod inquire_filepath_completer;
mod storages;
mod util;

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();
    trace!("Start logging...");
    trace!("args: {:?}", cli);

    let config_dir: std::path::PathBuf = match cli.config_dir {
        Some(path) => path,
        None => {
            let mut config_dir =
                dirs::config_local_dir().context("Failed to get default config dir.")?;
            config_dir.push("xdbm");
            config_dir
        }
    };
    trace!("Config dir: {:?}", config_dir);

    match cli.command {
        Commands::Init {
            device_name,
            repo_url,
            use_sshagent,
            ssh_key,
        } => cmd_init::cmd_init(device_name, repo_url, use_sshagent, ssh_key, &config_dir)?,
        Commands::Storage(storage) => {
            let repo = Repository::open(&config_dir).context(
                "Repository doesn't exist on the config path. Please run init to initialize the repository.",
            )?;
            trace!("repo state: {:?}", repo.state());
            match storage.command {
                StorageCommands::Add(storageargs) => {
                    cmd_storage::cmd_storage_add(storageargs.command, repo, &config_dir)?
                }
                StorageCommands::List { long } => cmd_storage::cmd_storage_list(&config_dir, long)?,
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
        Commands::Sync { remote_name } => cmd_sync::cmd_sync(&config_dir, remote_name)?,
        Commands::Check {} => {
            println!("Config dir: {}", &config_dir.display());
            let _storages = Storages::read(&config_dir)?;
            todo!()
        }
        Commands::Backup(backup) => {
            trace!("backup subcommand with args: {:?}", backup);
            let repo = Repository::open(&config_dir).context(
                "Repository doesn't exist on the config path. Please run init to initialize the repository.",
            )?;
            let storages = Storages::read(&config_dir)?;
            match backup {
                BackupSubCommands::Add {
                    name,
                    src,
                    dest,
                    cmd,
                } => cmd_backup::cmd_backup_add(name, src, dest, cmd, repo, &config_dir, &storages)?,
                BackupSubCommands::List {} => todo!(),
                BackupSubCommands::Done {
                    name,
                    exit_status,
                    log,
                } => todo!(),
            }
        }
    }
    full_status(&Repository::open(&config_dir)?)?;
    Ok(())
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
