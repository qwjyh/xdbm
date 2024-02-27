//! Init subcommand.
//! Initialize xdbm for the device.

use crate::{
    add_and_commit, full_status, get_devices, set_device_name, write_devices, Device, DEVICESFILE,
};
use anyhow::{anyhow, Context, Result};
use git2::Repository;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{self, Path};

pub(crate) fn cmd_init(repo_url: Option<String>, config_dir: &path::PathBuf) -> Result<()> {
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
                add_and_commit(&repo, Path::new(".gitignore"), "Add devname to gitignore.")?;
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
    Ok(())
}
