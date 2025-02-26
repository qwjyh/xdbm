use std::path::{Path, PathBuf};

use git2::{Cred, RemoteCallbacks};
use inquire::Password;

pub(crate) fn get_credential<'a>(
    use_sshagent: bool,
    ssh_key: Option<PathBuf>,
) -> RemoteCallbacks<'a> {
    // using credentials
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
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
    callbacks
}
