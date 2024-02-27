//! # Main types
//! * [Device]: represents PC.
//! * [Storage]: all storages. module [storages]
//!     * [PhysicalDrivePartition]: partition on a physical disk. [storages::physical_drive_partition]
//!     * [Directory]: sub-directory of other storages. [storages::directory]
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
use std::io::{self, BufWriter};
use std::path::{self, PathBuf};
use std::{env, io::BufReader, path::Path};
use std::{fmt::Debug, fs::File};
use std::{fs, io::prelude::*};
use sysinfo::{Disk, DiskExt, SystemExt};

use crate::cmd_args::{Cli, Commands, StorageArgs, StorageCommands};
use crate::inquire_filepath_completer::FilePathCompleter;
use crate::storages::online_storage::OnlineStorage;
use crate::storages::{
    directory::Directory, get_storages, local_info, online_storage, physical_drive_partition::*,
    write_storages, Storage, StorageExt, StorageType, STORAGESFILE,
};
use devices::{Device, DEVICESFILE, *};

mod cmd_args;
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
        Commands::Init { repo_url } => {
            let is_first_device: bool;
            // get repo or initialize it
            let repo = match repo_url {
                Some(repo_url) => {
                    trace!("repo: {}", repo_url);
                    let repo = Repository::clone(&repo_url, &config_dir)?;
                    is_first_device = false;
                    repo
                }
                None => {
                    trace!("No repo provided");
                    println!("Initializing for the first device...");

                    // create repository
                    let repo = Repository::init(&config_dir)?;

                    // set up gitignore
                    {
                        let f = File::create(&config_dir.join(".gitignore"))?;
                        {
                            let mut buf = BufWriter::new(f);
                            buf.write("devname".as_bytes())?;
                        }
                        add_and_commit(
                            &repo,
                            Path::new(".gitignore"),
                            "Add devname to gitignore.",
                        )?;
                        full_status(&repo)?;
                    }
                    is_first_device = true;
                    repo
                }
            };
            full_status(&repo)?;

            // set device name
            let device = set_device_name()?;

            // save devname
            let devname_path = &config_dir.join("devname");
            {
                let f = File::create(devname_path)
                    .context("Failed to create a file to store local device name")?;
                let writer = BufWriter::new(f);
                serde_yaml::to_writer(writer, &device.name()).unwrap();
            };
            full_status(&repo)?;

            // Add new device to devices.yml
            {
                let mut devices: Vec<Device> = if is_first_device {
                    vec![]
                } else {
                    get_devices(&config_dir)?
                };
                trace!("devices: {:?}", devices);
                if devices.iter().any(|x| x.name() == device.name()) {
                    return Err(anyhow!("device name is already used."));
                }
                devices.push(device.clone());
                trace!("Devices: {:?}", devices);
                write_devices(&config_dir, devices)?;
            }
            full_status(&repo)?;

            // commit
            add_and_commit(
                &repo,
                &Path::new(DEVICESFILE),
                &format!("Add new devname: {}", &device.name()),
            )?;
            println!("Device added");
            full_status(&repo)?;
        }
        Commands::Storage(storage) => {
            let repo = Repository::open(&config_dir).context(
                "Repository doesn't exist. Please run init to initialize the repository.",
            )?;
            trace!("repo state: {:?}", repo.state());
            match storage.command {
                StorageCommands::Add { storage_type, path } => {
                    trace!("Storage Add {:?}, {:?}", storage_type, path);
                    // Get storages
                    // let mut storages: Vec<Storage> = get_storages(&config_dir)?;
                    let mut storages: HashMap<String, Storage> = get_storages(&config_dir)?;
                    trace!("found storages: {:?}", storages);

                    let device = get_device(&config_dir)?;
                    let (key, storage) = match storage_type {
                        StorageType::Physical => {
                            let use_sysinfo = {
                                let options = vec![
                                    "Fetch disk information automatically.",
                                    "Type disk information manually.",
                                ];
                                let ans = Select::new("Do you fetch disk information automatically? (it may take a few minutes)", options)
                                    .prompt().context("Failed to get response. Please try again.")?;
                                match ans {
                                    "Fetch disk information automatically." => true,
                                    _ => false,
                                }
                            };
                            let (key, storage) = if use_sysinfo {
                                // select storage
                                select_physical_storage(device, &storages)?
                            } else {
                                let mut name = String::new();
                                loop {
                                    name = Text::new("Name for the storage:")
                                        .with_validator(min_length!(0, "At least 1 character"))
                                        .prompt()
                                        .context("Failed to get Name")?;
                                    if storages.iter().all(|(k, _v)| k != &name) {
                                        break;
                                    }
                                    println!("The name {} is already used.", name);
                                }
                                let kind = Text::new("Kind of storage (ex. SSD):")
                                    .prompt()
                                    .context("Failed to get kind.")?;
                                let capacity: u64 = CustomType::<u64>::new("Capacity (byte):")
                                    .with_error_message("Please type number.")
                                    .prompt()
                                    .context("Failed to get capacity.")?;
                                let fs = Text::new("filesystem:")
                                    .prompt()
                                    .context("Failed to get fs.")?;
                                let is_removable = Confirm::new("Is removable")
                                    .prompt()
                                    .context("Failed to get is_removable")?;
                                let mount_path: path::PathBuf = PathBuf::from(
                                    Text::new("mount path:")
                                        .with_autocomplete(FilePathCompleter::default())
                                        .prompt()?,
                                );
                                let local_info =
                                    local_info::LocalInfo::new("".to_string(), mount_path);
                                (
                                    name.clone(),
                                    PhysicalDrivePartition::new(
                                        name,
                                        kind,
                                        capacity,
                                        fs,
                                        is_removable,
                                        local_info,
                                        &device,
                                    ),
                                )
                            };
                            println!("storage: {}: {:?}", key, storage);
                            (key, Storage::PhysicalStorage(storage))
                        }
                        StorageType::SubDirectory => {
                            if storages.is_empty() {
                                return Err(anyhow!("No storages found. Please add at least 1 physical storage first."));
                            }
                            let path = path.unwrap_or_else(|| {
                                let mut cmd = Cli::command();
                                cmd.error(
                                    ErrorKind::MissingRequiredArgument,
                                    "<PATH> is required with sub-directory",
                                )
                                .exit();
                            });
                            trace!("SubDirectory arguments: path: {:?}", path);
                            // Nightly feature std::path::absolute
                            let path = path.canonicalize()?;
                            trace!("canonicalized: path: {:?}", path);

                            let key_name = ask_unique_name(&storages, "sub-directory".to_string())?;
                            let notes = Text::new("Notes for this sub-directory:").prompt()?;
                            let storage = storages::directory::Directory::try_from_device_path(
                                key_name.clone(),
                                path,
                                notes,
                                &device,
                                &storages,
                            )?;
                            (key_name, Storage::SubDirectory(storage))
                        }
                        StorageType::Online => {
                            let path = path.unwrap_or_else(|| {
                                let mut cmd = Cli::command();
                                cmd.error(
                                    ErrorKind::MissingRequiredArgument,
                                    "<PATH> is required with sub-directory",
                                )
                                .exit();
                            });
                            let mut name = String::new();
                            loop {
                                name = Text::new("Name for the storage:")
                                    .with_validator(min_length!(0, "At least 1 character"))
                                    .prompt()
                                    .context("Failed to get Name")?;
                                if storages.iter().all(|(k, _v)| k != &name) {
                                    break;
                                }
                                println!("The name {} is already used.", name);
                            }
                            let provider = Text::new("Provider:")
                                .prompt()
                                .context("Failed to get provider")?;
                            let capacity: u64 = CustomType::<u64>::new("Capacity (byte):")
                                .with_error_message("Please type number.")
                                .prompt()
                                .context("Failed to get capacity.")?;
                            let alias = Text::new("Alias:")
                                .prompt()
                                .context("Failed to get provider")?;
                            let storage = OnlineStorage::new(
                                name.clone(),
                                provider,
                                capacity,
                                alias,
                                path,
                                &device,
                            );
                            (name, Storage::Online(storage))
                        }
                    };

                    // add to storages
                    storages.insert(key.clone(), storage);
                    trace!("updated storages: {:?}", storages);

                    // write to file
                    write_storages(&config_dir, storages)?;

                    // commit
                    add_and_commit(
                        &repo,
                        &Path::new(STORAGESFILE),
                        &format!("Add new storage(physical drive): {}", key),
                    )?;

                    println!("Added new storage.");
                    trace!("Finished adding storage");
                }
                StorageCommands::List {} => {
                    // Get storages
                    let storages: HashMap<String, Storage> = get_storages(&config_dir)?;
                    trace!("found storages: {:?}", storages);
                    let device = get_device(&config_dir)?;
                    for (k, storage) in &storages {
                        println!("{}: {}", k, storage);
                        println!("    {}", storage.mount_path(&device, &storages)?.display());
                        // println!("{}: {}", storage.shorttypename(), storage.name()); // TODO
                    }
                }
                StorageCommands::Bind {
                    storage: storage_name,
                    alias: new_alias,
                    path: mount_point,
                } => {
                    let device = get_device(&config_dir)?;
                    // get storages
                    let mut storages: HashMap<String, Storage> = get_storages(&config_dir)?;
                    let commit_comment = {
                        // find matching storage
                        let storage = &mut storages
                            .get_mut(&storage_name)
                            .context(format!("No storage has name {}", storage_name))?;
                        let old_alias = storage
                            .local_info(&device)
                            .context(format!("Failed to get LocalInfo for {}", storage.name()))?
                            .alias()
                            .clone();
                        // TODO: get mount path for directory automatically?
                        storage.bound_on_device(new_alias, mount_point, &device)?;
                        // trace!("storage: {}", &storage);
                        format!("{} to {}", old_alias, storage.name())
                    };
                    trace!("bound new system name to the storage");
                    trace!("storages: {:#?}", storages);

                    write_storages(&config_dir, storages)?;
                    // commit
                    add_and_commit(
                        &repo,
                        &Path::new(STORAGESFILE),
                        &format!(
                            "Bound new storage name to physical drive ({})",
                            commit_comment
                        ),
                    )?;
                    println!(
                        "Bound new storage name to physical drive ({})",
                        commit_comment
                    );
                }
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
