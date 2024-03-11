//! Storage subcommands.

use std::{
    collections::HashMap,
    io::{self, Write},
    path::{Path, PathBuf},
    string,
};

use anyhow::{anyhow, Context, Result};
use byte_unit::Byte;
use clap::{error::ErrorKind, CommandFactory};
use git2::Repository;
use inquire::{min_length, Confirm, CustomType, Select, Text};
use unicode_width::{self, UnicodeWidthStr};

use crate::{
    add_and_commit,
    cmd_args::Cli,
    devices::{self, Device},
    inquire_filepath_completer::FilePathCompleter,
    storages::{
        self, directory, local_info, physical_drive_partition, Storage, StorageExt, StorageType,
        Storages,
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
    let mut storages = Storages::read(&config_dir)?;
    trace!("found storages: {:?}", storages);

    let device = devices::get_device(&config_dir)?;
    let storage = match storage_type {
        StorageType::P => {
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
            let storage = if use_sysinfo {
                // select storage
                physical_drive_partition::select_physical_storage(device, &storages)?
            } else {
                let mut name = String::new();
                loop {
                    name = Text::new("Name for the storage:")
                        .with_validator(min_length!(0, "At least 1 character"))
                        .prompt()
                        .context("Failed to get Name")?;
                    if storages.list.iter().all(|(k, _v)| k != &name) {
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
                physical_drive_partition::PhysicalDrivePartition::new(
                    name,
                    kind,
                    capacity,
                    fs,
                    is_removable,
                    local_info,
                    &device,
                )
            };
            println!("storage: {}: {:?}", storage.name(), storage);
            Storage::PhysicalStorage(storage)
        }
        StorageType::S => {
            if storages.list.is_empty() {
                return Err(anyhow!(
                    "No storages found. Please add at least 1 physical/online storage first to add sub directory."
                ));
            }
            let path = path.unwrap_or_else(|| {
                let mut cmd = Cli::command();
                // TODO: weired def of cmd argument
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
                key_name, path, notes, &device, &storages,
            )?;
            Storage::SubDirectory(storage)
        }
        StorageType::O => {
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
                if storages.list.iter().all(|(k, _v)| k != &name) {
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
                name, provider, capacity, alias, path, &device,
            );
            Storage::Online(storage)
        }
    };

    // add to storages
    let new_storage_name = storage.name().clone();
    storages.add(storage)?;
    trace!("updated storages: {:?}", storages);

    // write to file
    storages.write(&config_dir)?;

    // commit
    add_and_commit(
        &repo,
        &Path::new(storages::STORAGESFILE),
        &format!("Add new storage(physical drive): {}", new_storage_name),
    )?;

    println!("Added new storage.");
    trace!("Finished adding storage");
    Ok(())
}

pub(crate) fn cmd_storage_list(config_dir: &PathBuf, with_note: bool) -> Result<()> {
    // Get storages
    let storages = Storages::read(&config_dir)?;
    trace!("found storages: {:?}", storages);
    let device = devices::get_device(&config_dir)?;
    let mut stdout = io::BufWriter::new(io::stdout());
    write_storages_list(&mut stdout, &storages, &device, with_note)?;
    stdout.flush()?;
    Ok(())
}

fn write_storages_list(
    mut writer: impl io::Write,
    storages: &Storages,
    device: &Device,
    long_display: bool,
) -> Result<()> {
    let name_width = storages
        .list
        .iter()
        .map(|(_k, v)| v.name().width())
        .max()
        .unwrap();
    trace!("name widths: {}", name_width);
    for (_k, storage) in &storages.list {
        let size_str = match storage.capacity() {
            Some(b) => Byte::from_bytes(b.into())
                .get_appropriate_unit(true)
                .format(0)
                .to_string(),
            None => "".to_string(),
        };
        let isremovable = if let Storage::PhysicalStorage(s) = storage {
            if s.is_removable() {
                "+"
            } else {
                "-"
            }
        } else {
            " "
        };
        let path = storage.mount_path(&device, &storages).map_or_else(
            |e| {
                info!("Not found: {}", e);
                "".to_string()
            },
            |v| v.display().to_string(),
        );
        let parent_name = if let Storage::SubDirectory(s) = storage {
            s.parent(&storages)?
                .context(format!("Failed to get parent of storage {}", s))?
                .name()
        } else {
            ""
        };
        writeln!(
            writer,
            "{stype}{isremovable}: {name:<name_width$} {size:>8} {parent:<name_width$} {path}",
            stype = storage.shorttypename(),
            isremovable = isremovable,
            name = storage.name(),
            size = size_str,
            parent = parent_name,
            path = path,
        )?;
        if long_display {
            let note = match storage {
                Storage::PhysicalStorage(s) => s.kind(),
                Storage::SubDirectory(s) => &s.notes,
                Storage::Online(s) => &s.provider,
            };
            writeln!(writer, "    {}", note)?;
        }
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
    let device = devices::get_device(&config_dir)?;
    // get storages
    let mut storages = Storages::read(&config_dir)?;
    let commit_comment = {
        // find matching storage
        let storage = &mut storages
            .list
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

    storages.write(&config_dir)?;
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

fn ask_unique_name(storages: &Storages, target: String) -> Result<String> {
    let mut disk_name = String::new();
    loop {
        disk_name = Text::new(format!("Name for {}:", target).as_str()).prompt()?;
        if storages.list.iter().all(|(k, _v)| k != &disk_name) {
            break;
        }
        println!("The name {} is already used.", disk_name);
    }
    Ok(disk_name)
}
