#[macro_use]
extern crate log;

extern crate dirs;

use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use git2::{Commit, IndexEntry, Oid, Repository};
use inquire::{
    validator::{StringValidator, Validation},
    Text,
};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::{self, BufWriter};
use std::{env, io::BufReader, path::Path};
use sysinfo::{System, SystemExt};

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

    /// Print config dir.
    Path {},
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Device {
    name: String,
    os_name: String,
    os_version: String,
    hostname: String,
}

const DEVICESFILE: &str = "devices.yml";

struct Storage {}

struct BackupLog {}

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

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();
    trace!("Start logging...");

    let mut config_dir = match dirs::config_local_dir() {
        Some(dir) => dir,
        None => return Err("Failed to get config dir.".to_string()),
    };
    config_dir.push("xdbm");
    trace!("Config dir: {:?}", config_dir);

    match cli.command {
        Commands::Init { repo_url } => {
            let is_first_device: bool;
            // get repo or initialize it
            let repo_url = match repo_url {
                Some(repo_url) => {
                    trace!("repo: {}", repo_url);
                    let repo = match Repository::clone(&repo_url, &config_dir) {
                        Ok(repo) => repo,
                        Err(e) => return Err(e.to_string()),
                    };
                    is_first_device = false;
                    repo
                }
                None => {
                    trace!("No repo provided");
                    println!("Initializing for the first device...");

                    // create repository
                    let repo = match Repository::init(&config_dir) {
                        Ok(repo) => repo,
                        Err(e) => return Err(e.to_string()),
                    };

                    // set up gitignore
                    {
                        let f = match File::create(&config_dir.join(".gitignore")) {
                            Ok(f) => f,
                            Err(e) => return Err(e.to_string()),
                        };
                        let mut buf = BufWriter::new(f);
                        match buf.write("devname".as_bytes()) {
                            Ok(_)  => trace!("successfully created ignore file"),
                            Err(e) => return Err(e.to_string()),
                        };
                        match add_and_commit(&repo, Path::new(".gitignore"), "Add devname to gitignore.") {
                            Ok(_) => (),
                            Err(e) => return Err(e.to_string()),
                        };
                    }
                    is_first_device = true;
                    repo
                }
            };

            // set device name
            let device = set_device_name()?;

            // save devname
            let devname_path = &config_dir.join("devname");
            {
                let mut f = match File::create(devname_path) {
                    Ok(f) => f,
                    Err(e) => panic!("Failed to create devname file: {}", e),
                };
                let mut writer = BufWriter::new(f);
                serde_yaml::to_writer(writer, &device.name).unwrap();
            };

            // Add new device to devices.yml
            {
                let mut devices: Vec<Device> = if is_first_device {
                    vec![]
                } else {
                    get_devices(&config_dir)?
                };
                devices.push(device.clone());
                trace!("Devices: {:?}", devices);
                write_devices(&config_dir, devices)?;
            }

            // commit
            match add_and_commit(
                &repo_url,
                &Path::new(DEVICESFILE),
                &format!("Add new devname: {}", &device.name),
            ) {
                Ok(_) => (),
                Err(e) => return Err(e.to_string()),
            }
        }
        Commands::Path {} => {
            println!("{}", &config_dir.display());
        }
    }
    Ok(())
}

/// Set device name interactively.
fn set_device_name() -> Result<Device, String> {
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
            return Err(err.to_string());
        }
    };

    let device = Device::new(device_name);
    trace!("Device information: {:?}", device);
    trace!("Serialized: \n{}", serde_yaml::to_string(&device).unwrap());

    return Ok(device);
}

/// Get `Vec<Device>` from yaml file in `config_dir`.
fn get_devices(config_dir: &Path) -> Result<Vec<Device>, String> {
    trace!("get_devices");
    let f = match File::open(config_dir.join(DEVICESFILE)) {
        Ok(f) => f,
        Err(e) => return Err(e.to_string()),
    };
    let reader = BufReader::new(f);
    let yaml: Vec<Device> = match serde_yaml::from_reader(reader) {
        Ok(yaml) => yaml,
        Err(e) => return Err(e.to_string()),
    };
    return Ok(yaml);
}

/// Write `devices` to yaml file in `config_dir`.
fn write_devices(config_dir: &Path, devices: Vec<Device>) -> Result<(), String> {
    trace!("write_devices");
    let f = match OpenOptions::new().create(true).write(true).open(config_dir.join(DEVICESFILE)) {
        Ok(f) => f,
        Err(e) => return Err(e.to_string()),
    };
    let writer = BufWriter::new(f);
    match serde_yaml::to_writer(writer, &devices) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
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
    let mut index = repo.index()?;
    index.add_path(path)?;
    let oid = index.write_tree()?;
    let config = git2::Config::open_default()?;
    let signature = git2::Signature::now(
        config.get_entry("user.name")?.value().unwrap(),
        config.get_entry("user.email")?.value().unwrap(),
    )?;
    trace!("git signature: {}", signature);
    let parent_commit = find_last_commit(&repo)?;
    let tree = repo.find_tree(oid)?;
    match parent_commit {
        Some(parent_commit) => repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        ),
        None => repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[]),
    }
}
