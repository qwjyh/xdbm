//! Manipulates each client device.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter};
use std::path::Path;
use sysinfo::System;

/// YAML file to store known devices.
pub const DEVICESFILE: &str = "devices.yml";

/// Represents each devices.
/// Identified by name, which is accessible from `name()`.
/// Store os name, os version and hostname as supplemental information.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Device {
    name: String,
    os_name: String,
    os_version: String,
    hostname: String,
}

impl Device {
    /// Create new `Device` of name `name`. Additional data is obtained via sysinfo.
    /// Filling fields which one failed to get is filled with "unknown".
    pub fn new(name: String) -> Device {
        Device {
            name,
            os_name: System::name().unwrap_or_else(|| {
                warn!("Failed to get OS name. Saving as \"unknown\".");
                "unknown".to_string()
            }),
            os_version: System::os_version().unwrap_or_else(|| {
                warn!("Failed to get OS version. Saving as \"unknown\".");
                "unknown".to_string()
            }),
            hostname: System::host_name().unwrap_or_else(|| {
                warn!("Failed to get hostname. Saving as \"unknown\".");
                "unknown".to_string()
            }),
        }
    }

    /// Get name.
    pub fn name(&self) -> String {
        self.name.to_string()
    }

    /// Get formatted string containing hostname and os.
    pub fn other_info(&self) -> String {
        format!("{}: ({})", self.hostname, self.os_name)
    }
}

/// Get devname of the device from file `devname`.
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
pub fn get_device(config_dir: &Path) -> Result<Device> {
    let devname = get_devname(config_dir)?;
    let devices = get_devices(config_dir)?;
    trace!("devname: {}", devname);
    trace!("devices: {:?}", devices);
    devices
        .into_iter()
        .find(|dev| dev.name() == devname)
        .context("Couldn't find Device in devices.yml")
}

/// Get `Vec<Device>` from yaml file in `config_dir`.
pub fn get_devices(config_dir: &Path) -> Result<Vec<Device>> {
    trace!("get_devices");
    let f =
        File::open(config_dir.join(DEVICESFILE)).context(format!("{} not found", DEVICESFILE))?;
    let reader = BufReader::new(f);
    let yaml: Vec<Device> =
        serde_yaml::from_reader(reader).context("Failed to parse devices.yml")?;
    Ok(yaml)
}

/// Write `devices` to yaml file in `config_dir`.
pub fn write_devices(config_dir: &Path, devices: Vec<Device>) -> Result<()> {
    trace!("write_devices");
    let f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(config_dir.join(DEVICESFILE))?;
    let writer = BufWriter::new(f);
    serde_yaml::to_writer(writer, &devices).map_err(|e| anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::Device;

    #[test]
    fn get_name() {
        let device = Device::new("test".to_string());
        assert_eq!("test".to_string(), device.name());
    }
}
