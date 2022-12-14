use std::env::VarError;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use command::Command;
use directories::BaseDirs;
use tracing::{debug, error};

use self::{check::CheckCommand, download::DownloadCommand, get_versions::GetVersionsCommand};
use rust_analyzer_downloader::services::downloader::Downloader;
use rust_analyzer_downloader::services::versions::Versions;

mod check;
mod command;
mod download;
mod get_versions;

#[derive(Debug, Subcommand)]
enum Commands {
    Download {
        #[clap(required = false, value_parser)]
        version: String,

        #[clap(short, long, required = false, value_parser, default_value_t=get_default_output_path())]
        output: String,
    },
    GetVersions {
        #[clap(short, long, required = false, value_parser, default_value_t = 3)]
        per_page: u32,
    },
    Check {
        #[clap(short, long, required = false, value_parser, default_value_t=get_default_output_path())]
        output: String,
        #[clap(short, long, required = false, value_parser, default_value_t = false)]
        nightly: bool,
        #[clap(short, long, required = false, value_parser, default_value_t = false)]
        download: bool,
    },
}

#[derive(Debug, Parser)]
#[clap(name = "rust-analyzer-downloader",about = "Downloads and gets versions for Rust Analyzer", long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    commands: Commands,
}

fn default_user_output_path() -> String {
    let base_dirs = BaseDirs::new().unwrap();
    let home_dir = base_dirs.home_dir();

    let mut buf = PathBuf::new();

    buf.push(home_dir);
    buf.push("bin");
    #[cfg(target_family = "windows")]
    buf.push("rust-analyzer.exe");
    #[cfg(target_family = "unix")]
    buf.push("rust-analyzer");

    buf.as_path().to_string_lossy().into()
}

fn get_default_output_path() -> String {
    let env = std::env::var("RAD_OUTPUT_PATH");

    match env {
        Ok(path) => path,
        Err(VarError::NotUnicode(os_str)) => {
            error!("RAD_OUTPUT_PATH is not unicode: {:?}", os_str);

            default_user_output_path()
        }
        Err(VarError::NotPresent) => {
            let path = default_user_output_path();
            debug!(
                path = path.as_str(),
                "RAD_OUTPUT_PATH not present, using default path"
            );

            path
        }
    }
}

// #[tracing::instrument]
pub async fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let client = reqwest::ClientBuilder::new().build()?;

    let future = match args.commands {
        Commands::Download { version, output } => {
            Box::pin(DownloadCommand::new(version, output, Downloader::new(client)).execute())
        }
        Commands::GetVersions { per_page } => {
            debug!("Fetching versions from GitHub Releases API");
            let result =
                Box::pin(GetVersionsCommand::new(Versions::new(client), per_page).execute());
            debug!("Fetching versions completed from GitHub Releases API");

            result
        }
        Commands::Check {
            output,
            nightly,
            download,
        } => Box::pin(
            CheckCommand::new(
                output,
                Downloader::new(client.clone()),
                Versions::new(client),
                download,
                nightly,
            )
            .execute(),
        ),
    };

    match future.await {
        Err(err) => Err(Box::new(err)),
        Ok(_) => Ok(()),
    }
}
