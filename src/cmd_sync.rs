use std::path::PathBuf;

use anyhow::{anyhow, Result};
use git2::Repository;

pub(crate) fn cmd_sync(config_dir: &PathBuf, remote_name: Option<String>) -> Result<()> {
    warn!("Experimental");
    let repo = Repository::open(config_dir)?;
    let remote_name = match remote_name {
        Some(remote_name) => remote_name,
        None => {
            let remotes = repo.remotes()?;
            if remotes.len() != 1 {
                return Err(anyhow!("Please specify remote name"));
            }
            remotes.get(0).unwrap().to_string()
        }
    };
    let mut remote = repo.find_remote(&remote_name)?;
    remote.push(&[] as &[&str], None)?;
    Ok(())
}
