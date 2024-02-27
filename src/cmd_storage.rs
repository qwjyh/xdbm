use std::{path::{self, Path}, collections::HashMap, io::ErrorKind};

use anyhow::{Result, Context, Ok};
use clap::{CommandFactory, error};
use git2::Repository;
use inquire::Text;
use sysinfo::{SystemExt, DiskExt};

use crate::{anyhow, StorageCommands, cmd_options::{StorageArgs, Cli}, storages::{Storage, get_storages, StorageType, physical_drive_partition::{select_physical_storage, select_sysinfo_disk}, self, write_storages, STORAGESFILE, StorageExt}, devices::get_device, ask_unique_name, add_and_commit};

pub fn cmd_storage(storage: StorageArgs, config_dir: &path::PathBuf) -> Result<()> {
    let repo = Repository::open(&config_dir)
        .context("Repository doesn't exist. Please run init to initialize the repository.")?;
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
                    // select storage
                    let (key, storage) = select_physical_storage(device, &storages)?;
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
                            error::ErrorKind::MissingRequiredArgument,
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
                StorageType::Online => todo!(),
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
            Ok(())
        }
        StorageCommands::List {} => {
            // Get storages
            let storages: HashMap<String, Storage> = get_storages(&config_dir)?;
            trace!("found storages: {:?}", storages);
            let device = get_device(&config_dir)?;
            for (k, storage) in &storages {
                println!("{}: {}", k, storage);
                println!("    {}", storage.mount_path(&device, &storages)?.display());
            }
            Ok(())
        }
        StorageCommands::Bind {
            storage: storage_name,
        } => {
            // get storages
            let mut storages: HashMap<String, Storage> = get_storages(&config_dir)?;
            let commit_comment = {
                // find matching storage
                let storage = storages
                    .get_mut(&storage_name)
                    .context(format!("No storage has name {}", storage_name))?;
                // get disk from sysinfo
                let mut sysinfo = sysinfo::System::new_all();
                sysinfo.refresh_disks();
                let disk = select_sysinfo_disk(&sysinfo)?;
                let system_name = disk
                    .name()
                    .to_str()
                    .context("Failed to convert disk name to valid string")?;
                // add to storages
                storage.bind_device(disk, &config_dir)?;
                trace!("storage: {}", storage);
                format!("{} to {}", system_name, storage.name())
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
            Ok(())
        }
    }
}
