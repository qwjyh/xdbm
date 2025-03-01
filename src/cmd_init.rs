//! Init subcommand.
//! Initialize xdbm for the device.

use crate::backups::Backups;
use crate::storages::{STORAGESFILE, Storages};
use crate::{
    DEVICESFILE, Device, add_and_commit, backups,
    devices::{get_devices, write_devices},
    full_status,
};
use anyhow::{Context, Ok, Result, anyhow};
use core::panic;
use git2::{Cred, RemoteCallbacks, Repository};
use inquire::Password;
use std::fs::{DirBuilder, File};
use std::io::{BufWriter, Write};
use std::path::{self, Path, PathBuf};

fn clone_repo(
    repo_url: &str,
    use_sshagent: bool,
    ssh_key: Option<PathBuf>,
    config_dir: &path::PathBuf,
) -> Result<Repository> {
    // dont use credentials
    if ssh_key.is_none() && !use_sshagent {
        info!("No authentication will be used.");
        info!("Use either ssh_key or ssh-agent to access private repository");
        return Ok(Repository::clone(repo_url, config_dir)?);
    }

    // using credentials
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        if let Some(key) = &ssh_key {
            info!("Using provided ssh key to access the repository");
            let passwd = match Password::new("SSH passphrase").prompt() {
                std::result::Result::Ok(s) => Some(s),
                Err(err) => {
                    error!("Failed to get ssh passphrase: {:?}", err);
                    None
                }
            };
            Cred::ssh_key(
                username_from_url.ok_or(git2::Error::from_str("No username found from the url"))?,
                None,
                key as &Path,
                passwd.as_deref(),
            )
        } else if use_sshagent {
            // use ssh agent
            info!("Using ssh agent to access the repository");
            Cred::ssh_key_from_agent(
                username_from_url.ok_or(git2::Error::from_str("No username found from the url"))?,
            )
        } else {
            error!("no ssh_key and use_sshagent");
            panic!("This option must be unreachable.")
        }
    });

    // fetch options
    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    Ok(builder.clone(repo_url, config_dir)?)
}

pub(crate) fn cmd_init(
    device_name: String,
    repo_url: Option<String>,
    use_sshagent: bool,
    ssh_key: Option<PathBuf>,
    config_dir: &path::PathBuf,
) -> Result<()> {
    if config_dir.join(DEVICESFILE).exists() {
        debug!("{} already exists.", DEVICESFILE);
        return Err(anyhow!("This device is already added."));
    }
    // validate device name
    if device_name.chars().count() == 0 {
        log::error!("Device name cannot by empty");
        return Err(anyhow!("Device name is empty"));
    }
    // get repo or initialize it
    let repo = match repo_url {
        Some(repo_url) => {
            trace!("repo: {}", repo_url);
            clone_repo(&repo_url, use_sshagent, ssh_key, config_dir)?
        }
        None => {
            trace!("No repo provided");
            println!("Initializing for the first device...");

            // create repository
            let repo = Repository::init(config_dir)?;

            // set up gitignore
            {
                let f = File::create(config_dir.join(".gitignore"))?;
                {
                    let mut buf = BufWriter::new(f);
                    buf.write_all("devname".as_bytes())?;
                }
                add_and_commit(&repo, Path::new(".gitignore"), "Add devname to gitignore.")?;
                full_status(&repo)?;
            }

            // TDOO: wrap up below into one commit?
            // set up devices.yml
            let devices: Vec<Device> = vec![];
            write_devices(config_dir, devices)?;
            add_and_commit(
                &repo,
                Path::new(DEVICESFILE),
                &format!("Initialize {}", DEVICESFILE),
            )?;
            // set up storages.yml
            let storages = Storages::new();
            storages.write(config_dir)?;
            add_and_commit(
                &repo,
                Path::new(STORAGESFILE),
                &format!("Initialize {}", STORAGESFILE),
            )?;

            // set up directory for backups
            DirBuilder::new().create(config_dir.join(backups::BACKUPSDIR))?;

            repo
        }
    };
    full_status(&repo)?;

    // set device name
    // let device = set_device_name()?;
    let device = Device::new(device_name);
    trace!("Device information: {:?}", device);

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
        let mut devices: Vec<Device> = get_devices(config_dir)?;
        trace!("devices: {:?}", devices);
        if devices.iter().any(|x| x.name() == device.name()) {
            error!("Device name `{}` is already used.", device.name());
            error!("Clear the config directory and try again with different name");
            return Err(anyhow!("device name is already used."));
        }
        devices.push(device.clone());
        trace!("Devices: {:?}", devices);
        write_devices(config_dir, devices)?;
    }
    full_status(&repo)?;

    // commit
    add_and_commit(
        &repo,
        Path::new(DEVICESFILE),
        &format!("Add new device: {}", &device.name()),
    )?;

    // backups/[device].yml
    {
        let backups = Backups::new();
        backups.write(config_dir, &device)?;
    }
    add_and_commit(
        &repo,
        &backups::backups_file(&device),
        &format!("Add new backups for device: {}", &device.name()),
    )?;

    println!("Device added");
    full_status(&repo)?;
    Ok(())
}
