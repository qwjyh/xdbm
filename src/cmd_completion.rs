use crate::cmd_args::Cli;
use std::io;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

pub(crate) fn cmd_completion(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "xdbm", &mut io::stdout());
    Ok(())
}
