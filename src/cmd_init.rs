//! Init subcommand.
//! Initialize xdbm for the device.

use crate::{add_and_commit, full_status, get_devices, write_devices, Device, DEVICESFILE};
use anyhow::{anyhow, Context, Ok, Result};
use core::panic;
use git2::{Cred, RemoteCallbacks, Repository};
use inquire::Password;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{self, Path, PathBuf};

fn clone_repo(
    repo_url: &String,
    use_sshagent: bool,
    ssh_key: Option<PathBuf>,
    config_dir: &path::PathBuf,
) -> Result<Repository> {
    // dont use credentials
    if ssh_key.is_none() && !use_sshagent {
        info!("No authentication will be used.");
        info!("Use either ssh_key or ssh-agent to access private repository");
        return Ok(Repository::clone(&repo_url, &config_dir)?);
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
                username_from_url
                    .context("No username found from the url")
                    .unwrap(),
                None,
                &key as &Path,
                passwd.as_deref(),
            )
        } else if use_sshagent {
            // use ssh agent
            info!("Using ssh agent to access the repository");
            Cred::ssh_key_from_agent(
                username_from_url
                    .context("No username found from the url")
                    .unwrap(),
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

    Ok(builder.clone(&repo_url, config_dir)?)
}

pub(crate) fn cmd_init(
    device_name: String,
    repo_url: Option<String>,
    use_sshagent: bool,
    ssh_key: Option<PathBuf>,
    config_dir: &path::PathBuf,
) -> Result<()> {
    // validate device name
    if device_name.chars().count() == 0 {
        log::error!("Device name cannnot by empty");
        return Err(anyhow!("Device name is empty"));
    }
    // get repo or initialize it
    let (is_first_device, repo) = match repo_url {
        Some(repo_url) => {
            trace!("repo: {}", repo_url);
            let repo = clone_repo(&repo_url, use_sshagent, ssh_key, config_dir)?;
            (false, repo)
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
                add_and_commit(&repo, Path::new(".gitignore"), "Add devname to gitignore.")?;
                full_status(&repo)?;
            }
            (true, repo)
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
    Ok(())
}
