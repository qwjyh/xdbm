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
use clap::{Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::Verbosity;
use git2::{Commit, IndexEntry, Oid, Repository};
use inquire::{
    validator::Validation,
    Text,
};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{
    collections::{hash_map::RandomState, HashMap},
    fmt::Debug,
    fs::{File, OpenOptions},
};
use std::{env, io::BufReader, path::Path};
use std::{
    ffi::OsString,
    io::{self, BufWriter},
};
use std::{fs, io::prelude::*};
use sysinfo::{DiskExt, System, SystemExt};

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
    Add {
        #[arg(value_enum)]
        storage_type: StorageType,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Device {
    name: String,
    os_name: String,
    os_version: String,
    hostname: String,
}

impl Device {
    fn new(name: String) -> Device {
        let sys = System::new();
        Device {
            name: name,
            os_name: sys.name().unwrap_or_else(|| {
                warn!("Failed to get OS name. Saving as \"unknown\".");
                "unknown".to_string()
            }),
            os_version: sys.os_version().unwrap_or_else(|| {
                warn!("Failed to get OS version. Saving as \"unknown\".");
                "unknown".to_string()
            }),
            hostname: sys.host_name().unwrap_or_else(|| {
                warn!("Failed to get hostname. Saving as \"unknown\".");
                "unknown".to_string()
            }),
        }
    }
}

const DEVICESFILE: &str = "devices.yml";

#[derive(ValueEnum, Clone, Copy, Debug)]
enum StorageType {
    Physical,
    // Online,
}

/// All storage types.
#[derive(Serialize, Deserialize, Debug)]
enum Storage {
    PhysicalStorage(PhysicalDrivePartition),
    // /// Online storage provided by others.
    // OnlineStorage {
    //     name: String,
    //     provider: String,
    //     capacity: u8,
    // },
}

impl Storage {
    fn name(&self) -> &String {
        match self {
            Self::PhysicalStorage(s) => s.name(),
        }
    }
}

const STORAGESFILE: &str = "storages.yml";

/// Partitoin of physical (on-premises) drive.
#[derive(Serialize, Deserialize, Debug)]
struct PhysicalDrivePartition {
    name: String,
    kind: String,
    capacity: u64,
    fs: String,
    is_removable: bool,
    system_names: HashMap<String, String, RandomState>,
}

impl PhysicalDrivePartition {
    /// Try to get Physical drive info from sysinfo.
    fn try_from_sysinfo_disk(
        disk: &sysinfo::Disk,
        name: String,
        device: Device,
    ) -> Result<PhysicalDrivePartition> {
        let alias = disk
            .name()
            .to_str()
            .context("Failed to convert storage name to valid str.")?
            .to_string();
        let fs = disk.file_system();
        trace!("fs: {:?}", fs);
        let fs = std::str::from_utf8(fs)?;
        Ok(PhysicalDrivePartition {
            name: name,
            kind: format!("{:?}", disk.kind()),
            capacity: disk.total_space(),
            fs: fs.to_string(),
            is_removable: disk.is_removable(),
            system_names: HashMap::from([(device.name, alias)]),
        })
    }

    fn name(&self) -> &String {
        &self.name
    }

    fn add_alias(
        self,
        disk: sysinfo::Disk,
        device: Device,
    ) -> Result<PhysicalDrivePartition, String> {
        let alias = match disk.name().to_str() {
            Some(s) => s.to_string(),
            None => return Err("Failed to convert storage name to valid str.".to_string()),
        };
        let mut aliases = self.system_names;
        let _ = match aliases.insert(device.name, alias) {
            Some(v) => v,
            None => return Err("Failed to insert alias".to_string()),
        };
        Ok(PhysicalDrivePartition {
            name: self.name,
            kind: self.kind,
            capacity: self.capacity,
            fs: self.fs,
            is_removable: self.is_removable,
            system_names: aliases,
        })
    }
}

struct BackupLog {}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();
    trace!("Start logging...");

    let mut config_dir = dirs::config_local_dir().context("Failed to get config dir.")?;
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
                serde_yaml::to_writer(writer, &device.name).unwrap();
            };
            full_status(&repo)?;

            // Add new device to devices.yml
            {
                let mut devices: Vec<Device> = if is_first_device {
                    vec![]
                } else {
                    get_devices(&config_dir)?
                };
                if devices.iter().any(|x| x.name == device.name) {
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
                &format!("Add new devname: {}", &device.name),
            )?;
            full_status(&repo)?;
        },
        Commands::Storage(storage) => match storage.command {
            StorageCommands::Add { storage_type } => {
                trace!("Storage Add {:?}", storage_type);
                let repo = Repository::open(&config_dir)?;
                trace!("repo state: {:?}", repo.state());
                match storage_type {
                    StorageType::Physical => {
                        // Get storages
                        let mut storages: Vec<Storage> = if let Some(storages_file) = fs::read_dir(&config_dir)?
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
                            get_storages(&config_dir)?
                        } else {
                            trace!("No {} found", STORAGESFILE);
                            vec![]
                        };
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
                        add_and_commit(&repo, &Path::new(STORAGESFILE), &format!("Add new storage(physical drive): {}", new_storage_name))?;

                        println!("Added new storage.");
                        trace!("Finished adding storage");
                    }
                }
            }
        },
        Commands::Path {} => {
            println!("{}", &config_dir.display());
        },
        Commands::Sync {} => {
            unimplemented!("Sync is not implemented")
        },
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
        .filter(|dev| dev.name == devname)
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
fn select_physical_storage(device: Device, storages: &Vec<Storage>) -> Result<PhysicalDrivePartition> {
    trace!("select_physical_storage");
    // get disk info fron sysinfo
    let mut sys_disks = sysinfo::System::new_all();
    sys_disks.refresh_disks();
    trace!("Available disks");
    for disk in sys_disks.disks() {
        trace!("{:?}", disk)
    }
    let available_disks = sys_disks
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
    let (_, disk) = sys_disks
        .disks()
        .iter()
        .enumerate()
        .find(|(i, _)| i == &disk_num)
        .unwrap();
    trace!("selected disk: {:?}", disk);
    // name the disk
    let mut disk_name = String::new();
    trace!("{}", disk_name);
    loop {
        disk_name = Text::new("Name for the disk:").prompt()?;
        if storages.iter().all(|s| {s.name() != &disk_name}) {
            break;
        }
        println!("The name {} is already used.", disk_name);
    };
    trace!("selected name: {}", disk_name);
    PhysicalDrivePartition::try_from_sysinfo_disk(disk, disk_name, device)
}

/// Get Vec<Storage> from devices.yml([DEVICESFILE])
fn get_storages(config_dir: &Path) -> Result<Vec<Storage>> {
    let f = File::open(config_dir.join(STORAGESFILE))?;
    let reader = BufReader::new(f);
    // for line in reader.lines() {
    //     trace!("{:?}", line);
    // }
    // unimplemented!();
    let yaml: Vec<Storage> =
        serde_yaml::from_reader(reader).context("Failed to read devices.yml")?;
    Ok(yaml)
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
