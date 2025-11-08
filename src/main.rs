use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process;

// A dd-like tool with a progress bar.
#[derive(Parser, Debug)]
#[clap(name = "dd", author, version, about, long_about = None)]
struct Args {
    /// The input file.
    #[arg(short = 'i', long, value_name = "FILE")]
    input: PathBuf,

    /// The output file.
    #[arg(short = 'o', long, value_name = "FILE")]
    output: PathBuf,

    /// The block size in bytes.
    #[arg(short = 'b', long, default_value = "4096")]
    block_size: usize,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Open the input and output files.
    let mut input_file = File::open(&args.input)?;

    let mut output_file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&args.output)?;

    // Get the total size of the input file to create a properly sized progress bar.
    let total_size = input_file.metadata()?.len();

    // Create a new progress bar.
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );

    // Create a buffer for copying data in blocks.
    let mut buffer = vec![0; args.block_size];

    loop {
        let mut bytes_read = 0;
        while bytes_read < args.block_size {
            let n = match input_file.read(&mut buffer[bytes_read..]) {
                Ok(0) => break, // Reached end of file
                Ok(b_read) => b_read,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue, // Try read again
                Err(e) => {
                    eprintln!("Error reading from input file: {}", e);
                    process::exit(1);
                }
            };
            bytes_read += n;
        }

        if bytes_read == 0 {
            break; // End of file
        }

        output_file.write_all(&buffer[..bytes_read])?;
        output_file.sync_data()?;
        pb.inc(bytes_read as u64);
    }

    pb.finish_with_message("Copy complete!");
    println!();
    Ok(())
}
