//! Storage subcommands.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use clap::{error::ErrorKind, CommandFactory};
use git2::Repository;
use inquire::{min_length, Confirm, CustomType, Select, Text};

use crate::{
    add_and_commit, ask_unique_name,
    cmd_args::Cli,
    get_device,
    inquire_filepath_completer::FilePathCompleter,
    storages::{
        self, directory, get_storages, local_info, physical_drive_partition, Storage, StorageExt,
        StorageType,
    },
};

pub(crate) fn cmd_storage_add(
    storage_type: storages::StorageType,
    path: Option<PathBuf>,
    repo: Repository,
    config_dir: &PathBuf,
) -> Result<()> {
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
                let ans = Select::new(
                    "Do you fetch disk information automatically? (it may take a few minutes)",
                    options,
                )
                .prompt()
                .context("Failed to get response. Please try again.")?;
                match ans {
                    "Fetch disk information automatically." => true,
                    _ => false,
                }
            };
            let (key, storage) = if use_sysinfo {
                // select storage
                physical_drive_partition::select_physical_storage(device, &storages)?
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
                let mount_path: PathBuf = PathBuf::from(
                    Text::new("mount path:")
                        .with_autocomplete(FilePathCompleter::default())
                        .prompt()?,
                );
                let local_info = local_info::LocalInfo::new("".to_string(), mount_path);
                (
                    name.clone(),
                    physical_drive_partition::PhysicalDrivePartition::new(
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
                return Err(anyhow!(
                    "No storages found. Please add at least 1 physical storage first."
                ));
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
            let storage = directory::Directory::try_from_device_path(
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
            let storage = storages::online_storage::OnlineStorage::new(
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
    storages::write_storages(&config_dir, storages)?;

    // commit
    add_and_commit(
        &repo,
        &Path::new(storages::STORAGESFILE),
        &format!("Add new storage(physical drive): {}", key),
    )?;

    println!("Added new storage.");
    trace!("Finished adding storage");
    Ok(())
}

pub(crate) fn cmd_storage_list(config_dir: &PathBuf) -> Result<()> {
    // Get storages
    let storages: HashMap<String, Storage> = get_storages(&config_dir)?;
    trace!("found storages: {:?}", storages);
    let device = get_device(&config_dir)?;
    for (k, storage) in &storages {
        println!("{}: {}", k, storage);
        println!("    {}", storage.mount_path(&device, &storages)?.display());
        // println!("{}: {}", storage.shorttypename(), storage.name()); // TODO
    }
    Ok(())
}

pub(crate) fn cmd_storage_bind(
    storage_name: String,
    new_alias: String,
    mount_point: PathBuf,
    repo: Repository,
    config_dir: &PathBuf,
) -> Result<()> {
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

    storages::write_storages(&config_dir, storages)?;
    // commit
    add_and_commit(
        &repo,
        &Path::new(storages::STORAGESFILE),
        &format!(
            "Bound new storage name to physical drive ({})",
            commit_comment
        ),
    )?;
    println!(
        "Bound new storage name to physical drive ({})",
        commit_comment
    );
    Ok(())
}
