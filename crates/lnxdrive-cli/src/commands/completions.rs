//! Shell completions generation command
//!
//! Generates shell completions for bash, zsh, fish, elvish, and powershell.
//! Usage: `lnxdrive completions bash > ~/.local/share/bash-completion/completions/lnxdrive`

use std::io;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

use crate::output::OutputFormat;

/// Arguments for the completions subcommand
#[derive(Debug, clap::Args)]
pub struct CompletionsCommand {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

impl CompletionsCommand {
    /// Execute the completions command, printing completions to stdout
    pub async fn execute(&self, _format: OutputFormat) -> Result<()> {
        let mut cmd = crate::Cli::command();
        clap_complete::generate(self.shell, &mut cmd, "lnxdrive", &mut io::stdout());
        Ok(())
    }
}
