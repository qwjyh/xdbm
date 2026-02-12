//! # Main types
//! * [Device]: represents PC. module [devices]
//! * [storages::Storage]: all storages. module [storages]
//!     * [storages::physical_drive_partition::PhysicalDrivePartition]: partition on a physical disk. module [storages::physical_drive_partition]
//!     * [storages::directory::Directory]: sub-directory of other storages. module [storages::directory]
//!     * [storages::online_storage::OnlineStorage]: online storage like Google Drive. module [storages::online_storage]
//!     * [storages::Storages] for list of [storages::Storage]
//! * [storages::local_info::LocalInfo]: stores [Device] specific common data for [storages::Storage]s.
//! * [backups::Backup] for backup configuration and its logs.
//!     * [backups::BackupTarget] source and destination
//!     * [backups::BackupLog] backup log

#[macro_use]
extern crate log;

extern crate dirs;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;
use git2::{Commit, Oid, Repository};
use std::path::Path;
use std::path::{self, PathBuf};
use storages::Storages;

use crate::cmd_args::{BackupSubCommands, Cli, Commands, StorageCommands};
use devices::{DEVICESFILE, Device};

mod backups;
mod cmd_args;
mod cmd_backup;
mod cmd_check;
mod cmd_completion;
mod cmd_init;
mod cmd_status;
mod cmd_storage;
mod cmd_sync;
mod devices;
mod git;
mod inquire_filepath_completer;
mod storages;
mod util;

fn main() -> Result<()> {
    CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();
    trace!("Start logging...");
    trace!("args: {:?}", cli);

    let config_dir: std::path::PathBuf = match cli.config_dir {
        Some(path) => path,
        None => default_config_dir()?,
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
        Commands::Sync {
            remote_name,
            use_libgit2,
            use_sshagent,
            ssh_key,
        } => cmd_sync::cmd_sync(&config_dir, remote_name, use_sshagent, ssh_key, use_libgit2)?,
        Commands::Status {
            path,
            storage,
            backup,
        } => cmd_status::cmd_status(path, storage, backup, &config_dir)?,
        Commands::Check {} => cmd_check::cmd_check(&config_dir)?,
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
                } => {
                    cmd_backup::cmd_backup_add(name, src, dest, cmd, repo, &config_dir, &storages)?
                }
                BackupSubCommands::List {
                    src,
                    dest,
                    device,
                    long,
                } => cmd_backup::cmd_backup_list(src, dest, device, long, &config_dir, &storages)?,
                BackupSubCommands::Done {
                    name,
                    exit_status,
                    log,
                } => cmd_backup::cmd_backup_done(name, exit_status, log, repo, &config_dir)?,
            }
        }
        Commands::Completion { shell } => cmd_completion::cmd_completion(shell)?,
    }
    full_status(&Repository::open(&config_dir)?)?;
    Ok(())
}

fn default_config_dir() -> Result<PathBuf> {
    let mut config_dir = dirs::config_local_dir().context("Failed to get default config dir.")?;
    config_dir.push("xdbm");
    Ok(config_dir)
}

fn find_last_commit(repo: &'_ Repository) -> Result<Option<Commit<'_>>, git2::Error> {
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
    let config = repo.config()?;
    let signature = git2::Signature::now(
        config.get_entry("user.name")?.value().unwrap(),
        config.get_entry("user.email")?.value().unwrap(),
    )?;
    trace!("git signature: {}", signature);
    let parent_commit = find_last_commit(repo)?;
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
