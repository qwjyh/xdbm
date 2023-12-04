//! # Main variables
//! * [Device]: represents PC.
//! * [Storage]: all storages
//!     * [PhysicalDrivePartition]: partition on a physical disk.
//!

#[macro_use]
extern crate log;

extern crate dirs;

use anyhow::{anyhow, Context, Result};
use byte_unit::Byte;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use git2::{Commit, Oid, Repository};
use inquire::{validator::Validation, Text};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::HashMap;
use std::fmt::format;
use std::path::PathBuf;
use std::{env, io::BufReader, path::Path};
use std::{
    ffi::OsString,
    io::{self, BufWriter},
};
use std::{
    fmt::Debug,
    fs::{File, OpenOptions},
};
use std::{fs, io::prelude::*};
use sysinfo::{Disk, DiskExt, SystemExt};

use crate::storages::{
    get_storages, local_info, physical_drive_partition::*, write_storages, Storage, StorageExt,
    StorageType, STORAGESFILE,
};
use devices::{Device, DEVICESFILE};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[command(flatten)]
    verbose: Verbosity,
}

#[derive(Subcommand)]
enum Commands {
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
struct StorageArgs {
    #[command(subcommand)]
    command: StorageCommands,
}

#[derive(Subcommand)]
enum StorageCommands {
    /// Add new storage.
    Add {
        #[arg(value_enum)]
        storage_type: StorageType,

        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// List all storages.
    List {},
    /// Add new device-specific name to existing storage.
    /// For physical disk, the name is taken from system info automatically.
    Bind { storage: String },
}

mod devices;
mod storages;

struct BackupLog {}

#[feature(absolute_path)]
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

                    let (key, storage) = match storage_type {
                        StorageType::Physical => {
                            // select storage
                            let device = get_device(&config_dir)?;
                            let (key, storage) = select_physical_storage(device, &storages)?;
                            println!("storage: {}: {:?}", key, storage);
                            (key, Storage::PhysicalStorage(storage))
                        }
                        StorageType::SubDirectory => {
                            let mut storages: HashMap<String, Storage> = get_storages(&config_dir)?;
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
                            
                            // let (key, storage) = storages::directory::Directory::new(name, parent, relative_path, notes)
                            todo!()
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
                    for (k, storage) in storages {
                        println!("{}: {}", k, storage);
                    }
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
                        storage.add_alias(disk, &config_dir)?;
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
                }
            }
        }
        Commands::Path {} => {
            println!("{}", &config_dir.display());
        }
        Commands::Sync {} => {
            unimplemented!("Sync is not implemented")
        }
    }
    full_status(&Repository::open(&config_dir)?)?;
    Ok(())
}

/// Get devname of the device.
fn get_devname(config_dir: &Path) -> Result<String> {
    let f = File::open(config_dir.join("devname")).context("Failed to open devname file")?;
    let bufreader = BufReader::new(f);
    let devname = bufreader
        .lines()
        .next()
        .context("Couldn't get devname.")??;
    trace!("devname: {}", devname);
    Ok(devname)
}

/// Get current device.
fn get_device(config_dir: &Path) -> Result<Device> {
    let devname = get_devname(config_dir)?;
    let devices = get_devices(config_dir)?;
    trace!("devname: {}", devname);
    trace!("devices: {:?}", devices);
    devices
        .into_iter()
        .filter(|dev| dev.name() == devname)
        .next()
        .context("Couldn't find Device in devices.yml")
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

/// Get `Vec<Device>` from yaml file in `config_dir`.
fn get_devices(config_dir: &Path) -> Result<Vec<Device>> {
    trace!("get_devices");
    let f = File::open(config_dir.join(DEVICESFILE))?;
    let reader = BufReader::new(f);
    let yaml: Vec<Device> =
        serde_yaml::from_reader(reader).context("Failed to parse devices.yml")?;
    return Ok(yaml);
}

/// Write `devices` to yaml file in `config_dir`.
fn write_devices(config_dir: &Path, devices: Vec<Device>) -> Result<()> {
    trace!("write_devices");
    let f = OpenOptions::new()
        .create(true)
        .write(true)
        .open(config_dir.join(DEVICESFILE))?;
    let writer = BufWriter::new(f);
    serde_yaml::to_writer(writer, &devices).map_err(|e| anyhow!(e))
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
