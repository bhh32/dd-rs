use clap::Parser;
use std::path::PathBuf;

/// Convert and copy a file
#[derive(Parser, Debug)]
#[clap(name = "dd", author, version, about, long_about = None)]
pub struct Cli {
    /// Read from FILE instead of stdin.
    #[arg(long = "if", value_name = "FILE")]
    pub input: Option<PathBuf>,

    /// Write to FILE instead of stdout.
    #[arg(long = "of", value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Read and write up to BYTES bytes at a time (default: 512); overrides ibs and obs
    #[arg(long = "bs", default_value = "512")]
    pub block_size: usize,
}
