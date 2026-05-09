use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "etch")]
#[command(about = "Minimal timestamping blockchain. Nothing else.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    GenerateKey,
    SubmitPayload {
        #[arg(short, long)]
        payload: String,
        #[arg(short, long, default_value = ".")]
        data_dir: PathBuf,
    },
    Mine {
        #[arg(short, long)]
        secret: String,
        #[arg(short, long, default_value = "0.0.0.0:6262")]
        listen: String,
        #[arg(short, long)]
        peer: Vec<String>,
        #[arg(short, long, default_value = ".")]
        data_dir: PathBuf,
    },
    Node {
        #[arg(short, long, default_value = "0.0.0.0:6262")]
        listen: String,
        #[arg(short, long)]
        peer: Vec<String>,
        #[arg(short, long, default_value = ".")]
        data_dir: PathBuf,
    },
}
