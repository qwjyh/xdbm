//! Manipulates each client device.

use serde::{Deserialize, Serialize};
use sysinfo::{System, SystemExt};

/// YAML file to store known devices.
pub const DEVICESFILE: &str = "devices.yml";

/// Represents each devices.
/// Identified by name, which is accessible from `name()`.
/// Store os name, os version and hostname as supplimental information.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Device {
    name: String,
    os_name: String,
    os_version: String,
    hostname: String,
}

impl Device {
    /// Create new `Device` of name `name`. Additional data is obtained via sysinfo.
    pub fn new(name: String) -> Device {
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

    /// Get name.
    pub fn name(&self) -> String {
        self.name.to_string()
    }
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