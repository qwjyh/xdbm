use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use git2::{Cred, PushOptions, RemoteCallbacks, Repository};

pub(crate) fn cmd_sync(
    config_dir: &PathBuf,
    remote_name: Option<String>,
    use_sshagent: bool,
    ssh_key: Option<PathBuf>,
) -> Result<()> {
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

    // using credentials
    let mut callbacks = RemoteCallbacks::new();
    callbacks
        .credentials(|_url, username_from_url, _allowed_types| {
            if let Some(key) = &ssh_key {
                info!("Using provided ssh key to access the repository");
                let passwd = match inquire::Password::new("SSH passphrase").prompt() {
                    std::result::Result::Ok(s) => Some(s),
                    Err(err) => {
                        error!("Failed to get ssh passphrase: {:?}", err);
                        None
                    }
                };
                Cred::ssh_key(
                    username_from_url
                        .ok_or(git2::Error::from_str("No username found from the url"))?,
                    None,
                    key as &Path,
                    passwd.as_deref(),
                )
            } else if use_sshagent {
                // use ssh agent
                info!("Using ssh agent to access the repository");
                Cred::ssh_key_from_agent(
                    username_from_url
                        .ok_or(git2::Error::from_str("No username found from the url"))?,
                )
            } else {
                error!("no ssh_key and use_sshagent");
                panic!("This option must be unreachable.")
            }
        })
        .push_transfer_progress(|current, total, bytes| {
            trace!("{current},\t{total},\t{bytes}");
        });
    callbacks.push_update_reference(|reference_name, status_msg| {
        debug!("remote reference_name {reference_name}");
        match status_msg {
            None => {
                info!("successfully pushed");
                eprintln!("successfully pushed to {}", reference_name);
                Ok(())
            }
            Some(status) => {
                error!("failed to push: {}", status);
                Err(git2::Error::from_str(&format!(
                    "failed to push to {}: {}",
                    reference_name, status
                )))
            }
        }
    });
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    let mut remote = repo.find_remote(&remote_name)?;
    trace!("remote: {:?}", remote.name());
    if remote.refspecs().len() != 1 {
        warn!("multiple refspecs found");
    }
    trace!("refspec: {:?}", remote.get_refspec(0).unwrap().str());
    trace!("refspec: {:?}", remote.get_refspec(0).unwrap().direction());
    trace!("refspec: {:?}", repo.head().unwrap().name());
    trace!("head is branch: {:?}", repo.head().unwrap().is_branch());
    trace!("head is remote: {:?}", repo.head().unwrap().is_remote());
    remote.push(
        &[repo.head().unwrap().name().unwrap()] as &[&str],
        Some(&mut push_options),
    )?;
    Ok(())
}
