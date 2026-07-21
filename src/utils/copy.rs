use indicatif::ProgressBar;
use std::io::{self, Read, Write};
use std::process;

use super::{InputSource, OutputSource};

/// Create a buffer of the specified size
pub fn create_buffer(size: usize) -> Vec<u8> {
    vec![0; size]
}

/// Copy the data from input to output with callback for progress tracking
pub fn copy_with_callback<F>(
    input: &mut InputSource,
    output: &mut OutputSource,
    buffer_size: usize,
    mut callback: F,
) -> io::Result<()>
where
    F: FnMut(u64),
{
    let mut buffer = create_buffer(buffer_size);

    loop {
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => {
                output.write_all(&buffer[..bytes_read])?;
                callback(bytes_read as u64);
            }
            Err(e) => {
                eprintln!("Error reading from input file: {e}");
                process::exit(1);
            }
        }
    }

    Ok(())
}

/// Copy the data from input to output with progress tracking
pub fn copy_with_progress(
    input: &mut InputSource,
    output: &mut OutputSource,
    buffer_size: usize,
    pb: &ProgressBar,
) -> io::Result<()> {
    copy_with_callback(input, output, buffer_size, |bytes| {
        pb.inc(bytes);
    })
}
