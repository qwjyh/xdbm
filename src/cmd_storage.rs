//! Storage subcommands.

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use byte_unit::{Byte, UnitType};
use console::Style;
use dunce::canonicalize;
use git2::Repository;
use inquire::{Confirm, CustomType, Text};
use unicode_width::{self, UnicodeWidthStr};

use crate::{
    add_and_commit,
    cmd_args::StorageAddCommands,
    devices::{self, Device},
    storages::{
        self, directory, local_info,
        physical_drive_partition::{self, PhysicalDrivePartition},
        Storage, StorageExt, Storages,
    },
    util,
};

pub(crate) fn cmd_storage_add(
    args: StorageAddCommands,
    repo: Repository,
    config_dir: &Path,
) -> Result<()> {
    trace!("Storage Add with args: {:?}", args);
    // Get storages
    let mut storages = Storages::read(config_dir)?;
    trace!("found storages: {:?}", storages);

    let device = devices::get_device(config_dir)?;
    let storage = match args {
        StorageAddCommands::Physical { name, path } => {
            if !is_unique_name(&name, &storages) {
                return Err(anyhow!(
                    "The name {} is already used for another storage.",
                    name
                ));
            }
            let use_sysinfo = path.is_none();
            let storage = if use_sysinfo {
                physical_drive_partition::select_physical_storage(name, device)?
            } else {
                manually_construct_physical_drive_partition(
                    name,
                    canonicalize(util::expand_tilde(path.unwrap())?)?,
                    &device,
                )?
            };
            println!("storage: {}: {:?}", storage.name(), storage);
            Storage::Physical(storage)
        }
        StorageAddCommands::Directory {
            name,
            path,
            notes,
            alias,
        } => {
            if !is_unique_name(&name, &storages) {
                return Err(anyhow!(
                    "The name {} is already used for another storage.",
                    name
                ));
            }
            if storages.list.is_empty() {
                return Err(anyhow!(
                    "No storages found. Please add at least 1 physical/online storage first to add sub directory."
                ));
            }
            trace!("SubDirectory arguments: path: {:?}", path);
            // Nightly feature std::path::absolute
            trace!("Canonicalize path: {:?}", path);
            let path = canonicalize(util::expand_tilde(path)?)?;
            trace!("canonicalized: path: {:?}", path);

            let storage = directory::Directory::try_from_device_path(
                name, path, notes, alias, &device, &storages,
            )?;
            Storage::SubDirectory(storage)
        }
        StorageAddCommands::Online {
            name,
            path,
            provider,
            capacity,
            alias,
        } => {
            if !is_unique_name(&name, &storages) {
                return Err(anyhow!(
                    "The name {} is already used for another storage.",
                    name
                ));
            }
            trace!("Canonicalize path: {:?}", path);
            let path = canonicalize(util::expand_tilde(path)?)?;
            let storage = storages::online_storage::OnlineStorage::new(
                name, provider, capacity, alias, path, &device,
            );
            Storage::Online(storage)
        }
    };

    // add to storages
    let new_storage_name = storage.name().clone();
    let new_storage_type = storage.typename().to_string();
    storages.add(storage)?;
    trace!("updated storages: {:?}", storages);

    // write to file
    storages.write(config_dir)?;

    // commit
    add_and_commit(
        &repo,
        Path::new(storages::STORAGESFILE),
        &format!(
            "Add new storage({}): {}",
            new_storage_type, new_storage_name
        ),
    )?;

    println!("Added new storage.");
    trace!("Finished adding storage");
    Ok(())
}

fn is_unique_name(newname: &String, storages: &Storages) -> bool {
    storages.list.iter().all(|(name, _)| name != newname)
}

fn manually_construct_physical_drive_partition(
    name: String,
    path: PathBuf,
    device: &Device,
) -> Result<PhysicalDrivePartition> {
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
    let alias = Text::new("Alias of the storage for this device")
        .prompt()
        .context("Failed to get alias.")?;
    let local_info = local_info::LocalInfo::new(alias, path);
    Ok(physical_drive_partition::PhysicalDrivePartition::new(
        name,
        kind,
        capacity,
        fs,
        is_removable,
        local_info,
        device,
    ))
}

pub(crate) fn cmd_storage_list(config_dir: &Path, with_note: bool) -> Result<()> {
    // Get storages
    let storages = Storages::read(config_dir)?;
    trace!("found storages: {:?}", storages);
    if storages.list.is_empty() {
        println!("No storages found");
        return Ok(());
    }
    let device = devices::get_device(config_dir)?;
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
        .values()
        .map(|v| v.name().width())
        .max()
        .unwrap();
    trace!("name widths: {}", name_width);
    for storage in storages.list.values() {
        let size_str = match storage.capacity() {
            Some(b) => {
                let size = Byte::from_u64(b).get_appropriate_unit(UnitType::Binary);
                // TODO: split case for 500GB and 1.5TB?
                format!("{:>+5.1}", size)
            }
            None => "".to_string(),
        };
        let isremovable = if let Storage::Physical(s) = storage {
            if s.is_removable() {
                "+"
            } else {
                "-"
            }
        } else {
            " "
        };
        let path = storage.mount_path(device).map_or_else(
            |e| {
                info!("Not found: {}", e);
                "".to_string()
            },
            |v| v.display().to_string(),
        );
        let parent_name = if let Storage::SubDirectory(s) = storage {
            s.parent(storages)
                .context(format!("Failed to get parent of storage {}", s))?
                .name()
        } else {
            ""
        };
        writeln!(
            writer,
            "{stype}{isremovable}: {name:<name_width$} {size:>10} {parent:<name_width$} {path}",
            stype = storage.shorttypename(),
            isremovable = isremovable,
            name = storage.name(),
            size = size_str,
            parent = parent_name,
            path = path,
        )?;
        if long_display {
            let note = match storage {
                Storage::Physical(s) => s.kind(),
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
    config_dir: &Path,
) -> Result<()> {
    let device = devices::get_device(config_dir)?;
    // get storages
    let mut storages = Storages::read(config_dir)?;
    let commit_comment = {
        // find matching storage
        let storage = &mut storages
            .list
            .get_mut(&storage_name)
            .context(format!("No storage has name {}", storage_name))?;
        if let Some(localinfo) = storage.local_info(&device) {
            return Err(anyhow!(
                "The storage {} is already bounded on this device as {}",
                storage.name(),
                localinfo.alias(),
            ));
        }
        // TODO: get mount path for directory automatically?
        storage.bound_on_device(new_alias.clone(), mount_point, &device)?;
        // trace!("storage: {}", &storage);
        format!(
            "{} is now {} on the device {}",
            storage.name(),
            new_alias,
            device.name()
        )
    };
    trace!("bound new system name to the storage");
    trace!("storages: {:#?}", storages);

    storages.write(config_dir)?;
    // commit
    add_and_commit(
        &repo,
        Path::new(storages::STORAGESFILE),
        &format!("Bound new storage name to storage ({})", commit_comment),
    )?;
    println!("Bound new storage name to storage ({})", commit_comment);
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
