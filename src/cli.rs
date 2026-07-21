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

    /// Read and write up to BYTES bytes at a time (default: 4M); overrides ibs and obs
    #[arg(long = "bs", default_value = "4M", value_parser = parse_size)]
    pub block_size: usize,

    #[arg(long = "sha256", value_name = "HEX")]
    pub sha256: Option<String>,
}

fn parse_size(size: &str) -> Result<usize, String> {
    let size = size.trim();
    let (num, mult) = match size.chars().last() {
        Some('K') | Some('k') => (&size[..size.len() - 1], 1024),
        Some('M') | Some('m') => (&size[..size.len() - 1], 1024 * 1024),
        Some('G') | Some('g') => (&size[..size.len() - 1], 1024 * 1024 * 1024),
        Some('B') | Some('b') => (&size[..size.len() - 1], 1),
        _ => (size, 1),
    };

    num.trim()
        .parse::<usize>()
        .map_err(|_| format!("invalid size: '{size}'"))?
        .checked_mul(mult)
        .ok_or_else(|| format!("size is too large: '{size}'"))
}
