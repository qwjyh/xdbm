use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use git2::{build::CheckoutBuilder, Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository};

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
    debug!("remote name: {remote_name}");

    let mut remote = repo.find_remote(&remote_name)?;

    pull(
        &repo,
        &mut remote,
        remote_name,
        &use_sshagent,
        ssh_key.as_ref(),
    )?;

    push(&repo, &mut remote, &use_sshagent, ssh_key.as_ref())?;
    Ok(())
}

fn remote_callback<'b, 'a>(
    use_sshagent: &'a bool,
    ssh_key: Option<&'a PathBuf>,
) -> RemoteCallbacks<'a>
where
    'b: 'a,
{
    // using credentials
    let mut callbacks = RemoteCallbacks::new();
    callbacks
        .credentials(move |_url, username_from_url, _allowed_types| {
            if let Some(key) = ssh_key {
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
            } else if *use_sshagent {
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
        .transfer_progress(|progress| {
            if progress.received_objects() == progress.total_objects() {
                print!(
                    "Resolving deltas {}/{}\r",
                    progress.indexed_deltas(),
                    progress.total_deltas()
                );
            } else {
                print!(
                    "Received {}/{} objects ({}) in {} bytes\r",
                    progress.received_objects(),
                    progress.total_objects(),
                    progress.indexed_objects(),
                    progress.received_bytes(),
                );
            }
            io::stdout().flush().unwrap();
            true
        })
        .sideband_progress(|text| {
            let msg = String::from_utf8_lossy(text);
            eprintln!("remote: {msg}");
            true
        })
        .push_transfer_progress(|current, total, bytes| {
            trace!("{current}/{total} files sent \t{bytes} bytes");
        })
        .push_update_reference(|reference_name, status_msg| {
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
    callbacks
}

fn pull(
    repo: &Repository,
    remote: &mut git2::Remote,
    remote_name: String,
    use_sshagent: &bool,
    ssh_key: Option<&PathBuf>,
) -> Result<()> {
    let callbacks = remote_callback(use_sshagent, ssh_key);
    let mut fetchoptions = FetchOptions::new();
    fetchoptions.remote_callbacks(callbacks);
    let fetch_refspec: Vec<String> = remote
        .refspecs()
        .filter_map(|rs| match rs.direction() {
            git2::Direction::Fetch => rs.str().map(|s| s.to_string()),
            git2::Direction::Push => None,
        })
        .collect();
    remote
        .fetch(&fetch_refspec, Some(&mut fetchoptions), None)
        .context("Failed to fetch (pull)")?;
    let stats = remote.stats();
    if stats.local_objects() > 0 {
        println!(
            "\rReceived {}/{} objects in {} bytes (used {} local objects)",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes(),
            stats.local_objects(),
        );
    } else {
        println!(
            "\rReceived {}/{} objects in {} bytes",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes(),
        );
    }
    let fetch_head = repo
        .reference_to_annotated_commit(
            &repo
                .resolve_reference_from_short_name(&remote_name)
                .context("failed to get reference from fetch refspec")?,
        )
        .context("failed to get annotated commit")?;
    let (merge_analysis, merge_preference) = repo
        .merge_analysis(&[&fetch_head])
        .context("failed to do merge_analysis")?;

    trace!("merge analysis: {:?}", merge_analysis);
    trace!("merge preference: {:?}", merge_preference);
    match merge_analysis {
        ma if ma.is_up_to_date() => {
            info!("HEAD is up to date. skip merging");
        }
        ma if ma.is_fast_forward() => {
            // https://github.com/rust-lang/git2-rs/blob/master/examples/pull.rs
            info!("fast forward is available");
            let mut ref_remote = repo
                .find_reference(
                    remote
                        .default_branch()
                        .context("failed to get remote default branch")?
                        .as_str()
                        .unwrap(),
                )
                .context("failed to get remote reference")?;
            let name = match ref_remote.name() {
                Some(s) => s.to_string(),
                None => String::from_utf8_lossy(ref_remote.name_bytes()).to_string(),
            };
            let msg = format!("Fast-Forward: Setting {} to id: {}", name, fetch_head.id());
            println!("{}", msg);
            ref_remote
                .set_target(fetch_head.id(), &msg)
                .context("failed to set target")?;
            repo.checkout_head(Some(CheckoutBuilder::default().force()))
                .context("failed to checkout")?;
        }
        ma if ma.is_unborn() => {
            warn!("HEAD is invalid (unborn)");
            return Err(anyhow!(
                "HEAD is invalid: merge_analysis: {:?}",
                merge_analysis
            ));
        }
        ma if ma.is_none() => {
            error!("no merge is possible");
            return Err(anyhow!("no merge is possible"));
        }
        ma if ma.is_normal() => {
            error!("unable to fast-forward. manual merge is required");
            return Err(anyhow!("unable to fast-forward. manual merge is required"));
        }
        _ma => {
            error!(
                "this code must not reachable: merge_analysis {:?}",
                merge_analysis
            );
            return Err(anyhow!("must not be reachabel (uncovered merge_analysis)"));
        }
    }
    Ok(())
}

fn push(
    repo: &Repository,
    remote: &mut git2::Remote,
    use_sshagent: &bool,
    ssh_key: Option<&PathBuf>,
) -> Result<()> {
    let callbacks = remote_callback(&use_sshagent, ssh_key);
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    trace!("remote: {:?}", remote.name());
    let num_refspecs = remote
        .refspecs()
        .filter(|rs| rs.direction() == git2::Direction::Push)
        .count();
    if num_refspecs > 1 {
        warn!("more than one push refspecs are configured");
        warn!("using the first one");
    }
    let head = repo.head().context("Failed to get HEAD")?;
    if num_refspecs >= 1 {
        trace!("using push refspec");
        let push_refspec = remote
            .refspecs()
            .filter_map(|rs| match rs.direction() {
                git2::Direction::Fetch => None,
                git2::Direction::Push => Some(rs),
            })
            .next()
            .expect("this must be unreachabe")
            .str()
            .context("failed to get valid utf8 push refspec")?
            .to_string();
        remote.push(&[push_refspec.as_str()] as &[&str], Some(&mut push_options))?;
    } else {
        trace!("using head as push refspec");
        trace!("head is branch: {:?}", head.is_branch());
        trace!("head is remote: {:?}", head.is_remote());
        let push_refspec = head.name().context("failed to get head name")?;
        remote.push(&[push_refspec] as &[&str], Some(&mut push_options))?;
    };
    Ok(())
}
