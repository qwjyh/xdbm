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
use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use git2::{Commit, Oid, Repository};
use inquire::{validator::Validation, Text};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fmt::format;
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

use devices::{Device, DEVICESFILE};
use storages::{physical_drive_partition::*, Storage, StorageExt, StorageType, STORAGESFILE};

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
    /// Initialize
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
                let f = File::create(devname_path).context("Failed to create devname file")?;
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
                StorageCommands::Add { storage_type } => {
                    trace!("Storage Add {:?}", storage_type);
                    match storage_type {
                        StorageType::Physical => {
                            // Get storages
                            let mut storages: Vec<Storage> = get_storages(&config_dir)?;
                            trace!("found storages: {:?}", storages);

                            // select storage
                            let device = get_device(&config_dir)?;
                            let storage = select_physical_storage(device, &storages)?;
                            println!("storage: {:?}", storage);
                            let new_storage_name = storage.name().clone();

                            // add to storages
                            storages.push(Storage::PhysicalStorage(storage));
                            trace!("updated storages: {:?}", storages);

                            // write to file
                            write_storages(&config_dir, storages)?;

                            // commit
                            add_and_commit(
                                &repo,
                                &Path::new(STORAGESFILE),
                                &format!("Add new storage(physical drive): {}", new_storage_name),
                            )?;

                            println!("Added new storage.");
                            trace!("Finished adding storage");
                        }
                    }
                }
                StorageCommands::List {} => {
                    // Get storages
                    let storages: Vec<Storage> = get_storages(&config_dir)?;
                    trace!("found storages: {:?}", storages);
                    for storage in storages {
                        println!("{}", storage);
                    }
                }
                StorageCommands::Bind {
                    storage: storage_name,
                } => {
                    // get storages
                    let mut storages: Vec<Storage> = get_storages(&config_dir)?;
                    let commit_comment = {
                        // find matching storage
                        let storage = &mut storages
                            .iter_mut()
                            .find(|s| s.name() == &storage_name)
                            .context(format!("No storage has name {}", storage_name))?;
                        // get disk from sysinfo
                        let mut sysinfo = sysinfo::System::new_all();
                        sysinfo.refresh_disks();
                        let disk = select_sysinfo_disk(&sysinfo)?;
                        let system_name = disk.name().to_str().context("Failed to convert disk name to valid string")?;
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
                        &format!("Bound new storage name to physical drive ({})", commit_comment),
                    )?;
                    println!("Bound new storage name to physical drive ({})", commit_comment);
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

    let device_name = Text::new("Provide the device(PC) name:")
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

/// Interactively select physical storage from available disks in sysinfo.
fn select_physical_storage(
    device: Device,
    storages: &Vec<Storage>,
) -> Result<PhysicalDrivePartition> {
    trace!("select_physical_storage");
    // get disk info fron sysinfo
    let mut sys_disks = sysinfo::System::new_all();
    sys_disks.refresh_disks();
    trace!("Available disks");
    for disk in sys_disks.disks() {
        trace!("{:?}", disk)
    }
    let disk = select_sysinfo_disk(&sys_disks)?;
    // name the disk
    let mut disk_name = String::new();
    trace!("{}", disk_name);
    loop {
        disk_name = Text::new("Name for the disk:").prompt()?;
        if storages.iter().all(|s| s.name() != &disk_name) {
            break;
        }
        println!("The name {} is already used.", disk_name);
    }
    trace!("selected name: {}", disk_name);
    PhysicalDrivePartition::try_from_sysinfo_disk(&disk, disk_name, device)
}

fn select_sysinfo_disk(sysinfo: &sysinfo::System) -> Result<&Disk> {
    let available_disks = sysinfo
        .disks()
        .iter()
        .enumerate()
        .map(|(i, disk)| {
            let name = match disk.name().to_str() {
                Some(s) => s,
                None => "",
            };
            let fs: &str = std::str::from_utf8(disk.file_system()).unwrap_or("unknown");
            let kind = format!("{:?}", disk.kind());
            let mount_path = disk.mount_point();
            let total_space = Byte::from_bytes(disk.total_space().into())
                .get_appropriate_unit(true)
                .to_string();
            format!(
                "{}: {} {} ({}, {}) {}",
                i,
                name,
                total_space,
                fs,
                kind,
                mount_path.display()
            )
        })
        .collect();
    // select from list
    let disk = inquire::Select::new("Select drive:", available_disks).prompt()?;
    let disk_num: usize = disk.split(':').next().unwrap().parse().unwrap();
    trace!("disk_num: {}", disk_num);
    let disk = sysinfo
        .disks()
        .iter()
        .nth(disk_num)
        .context("no disk matched with selected one.")?;
    trace!("selected disk: {:?}", disk);
    Ok(disk)
}

/// Get `Vec<Storage>` from devices.yml([DEVICESFILE]).
/// If [DEVICESFILE] isn't found, return empty vec.
fn get_storages(config_dir: &Path) -> Result<Vec<Storage>> {
    if let Some(storages_file) = fs::read_dir(&config_dir)?
        .filter(|f| {
            f.as_ref().map_or_else(
                |_e| false,
                |f| {
                    let storagesfile: OsString = STORAGESFILE.into();
                    f.path().file_name() == Some(&storagesfile)
                },
            )
        })
        .next()
    {
        trace!("{} found: {:?}", STORAGESFILE, storages_file);
        let f = File::open(config_dir.join(STORAGESFILE))?;
        let reader = BufReader::new(f);
        let yaml: Vec<Storage> =
            serde_yaml::from_reader(reader).context("Failed to read devices.yml")?;
        Ok(yaml)
    } else {
        trace!("No {} found", STORAGESFILE);
        Ok(vec![])
    }
}

/// Write `storages` to yaml file in `config_dir`.
fn write_storages(config_dir: &Path, storages: Vec<Storage>) -> Result<()> {
    let f = File::create(config_dir.join(STORAGESFILE))?;
    let writer = BufWriter::new(f);
    serde_yaml::to_writer(writer, &storages).map_err(|e| anyhow!(e))
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

/// Print git repo status
fn full_status(repo: &Repository) -> Result<()> {
    trace!("status: ");
    for status in repo.statuses(None)?.iter() {
        let path = status.path().unwrap_or("");
        let st = status.status();
        trace!("  {}: {:?}", path, st);
    }
    Ok(())
}
