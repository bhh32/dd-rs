use indicatif::{ProgressBar, ProgressStyle};
use rand::{RngCore, rng};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write, stdin, stdout},
    path::PathBuf,
    process,
};
use sysinfo::System;

/// Represents either a file or stdin for input
pub enum InputSource {
    File(File),
    Stdin(io::Stdin),
    DevZero,
    DevNull,
    DevUrandom,
}

impl Read for InputSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            InputSource::File(file) => file.read(buf),
            InputSource::Stdin(stdin) => stdin.read(buf),
            InputSource::DevZero => {
                // Fill buffer with zeros
                buf.fill(0);
                Ok(buf.len())
            }
            InputSource::DevNull => {
                // Always return EOF (0 bytes read)
                Ok(0)
            }
            InputSource::DevUrandom => {
                // Fill buffer with random bytes
                rng().fill_bytes(buf);
                Ok(buf.len())
            }
        }
    }
}

/// Represents either a file or stdout for output
pub enum OutputSource {
    File(File),
    Stdout(io::Stdout),
    DevNull,
    DevFull,
}

impl Write for OutputSource {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            OutputSource::File(file) => file.write(buf),
            OutputSource::Stdout(stdout) => stdout.write(buf),
            OutputSource::DevNull => {
                // Discard all data, claim it was written
                Ok(buf.len())
            }
            OutputSource::DevFull => {
                // Simulate device full error
                Err(io::Error::new(
                    io::ErrorKind::StorageFull,
                    "No space left on device",
                ))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            OutputSource::File(file) => file.flush(),
            OutputSource::Stdout(stdout) => stdout.flush(),
            OutputSource::DevNull | OutputSource::DevFull => Ok(()), // Nothing to flush
        }
    }
}

/// Detects if a path refers to a special device
fn is_special_dev(path: &PathBuf) -> bool {
    let path_str = path.to_string_lossy();
    matches!(
        path_str.as_ref(),
        "/dev/null" | "/dev/zero" | "/dev/urandom" | "/dev/random" | "/dev/full"
    )
}

/// Validates that special device isn't being "copied" to special device
pub fn validate_special_device_combo(
    input_path: Option<&PathBuf>,
    output_path: Option<&PathBuf>,
) -> io::Result<()> {
    match (input_path, output_path) {
        (Some(input), Some(output)) => {
            if is_special_dev(input) && is_special_dev(output) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Cannot copoy from special device {} to special device {}",
                        input.display(),
                        output.display()
                    ),
                ));
            }
        }
        _ => { /* Do nothing, it's valid */ }
    }

    Ok(())
}

/// Opens an input file for reading
pub fn open_input_file(path: Option<&PathBuf>) -> io::Result<InputSource> {
    match path {
        Some(path) => {
            let path_str = path.to_string_lossy();
            match path_str.as_ref() {
                "/dev/null" => Ok(InputSource::DevNull),
                "/dev/zero" => Ok(InputSource::DevZero),
                "/dev/urandom" | "/dev/random" => Ok(InputSource::DevUrandom),
                _ => Ok(InputSource::File(File::open(path)?)),
            }
        }
        None => Ok(InputSource::Stdin(stdin())),
    }
}

/// Opens an output file for writing
pub fn open_output_file(path: Option<&PathBuf>) -> io::Result<OutputSource> {
    match path {
        Some(path) => {
            let path_str = path.to_string_lossy();
            match path_str.as_ref() {
                "/dev/null" => Ok(OutputSource::DevNull),
                "/dev/full" => Ok(OutputSource::DevFull),
                _ => Ok(OutpuSource::File(
                    OpenOptions::new().write(true).create(true).open(path)?,
                )),
            }
        }
        None => Ok(OutputSource::Stdout(stdout())),
    }
}

/// Gets available space on filesystem for a file path
fn get_avialable_space(path: &PathBuf) -> io::Result<Option<u64>> {
    let mut sys = System::new();
    sys.refresh_disks_list();

    // Get the absolute path to properly match against disk mount points
    let abs_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // If path doesn't exist, try the parent directory
            match path.parent() {
                Some(parent) => match parent.canonicalize() {
                    Ok(p) => p,
                    Err(_) => return Ok(None),
                },
                None => return Ok(None),
            }
        }
    };

    // Find the disk that contains this path
    let mut best_match = None;
    let mut best_match_len = 0;

    for disk in sys.disks() {
        let mount_point = disk.mount_point();
        if abs_path.starts_with(mount_point) {
            let mount_point_len = mount_point.as_os_str().len();
            if mount_point_len > best_match_len {
                best_match = Some(disk);
                best_match_len = mount_point_len;
            }
        }
    }

    match best_match {
        Some(disk) => Ok(Some(disk.available_space())),
        None => Ok(None),
    }
}

/// Get the progress target size based on input/output combination
pub fn get_progress_target(
    input: &InputSource,
    output: &OutputSource,
    output_path: Option<&PathBuf>,
) -> io::Result<(Option<u64>, ProgressType)> {
    match input {
        InputSource::File(file) => {
            // Regular file input - use file size
            let size = file.metadata()?.len();
            Ok((Some(size), ProgressType::FileTransfer))
        }
        InputSource::Stdin(_) => {
            // Stdin - unknown size
            Ok((None, ProgressType::StreamTransfer))
        }
        InputSource::DevNull => {
            // /dev/null input - immediate EOF
            Ok((Some(0), ProgressType::FileTransfer))
        }
        InputSource::DevZero => {
            // Infinite zeros - base on output capacity
            match output {
                OutputSource::File(_) => {
                    if let Some(path) = output_path {
                        let available = get_available_space(path)?;
                        Ok((available, ProgressType::FillWithZeros))
                    } else {
                        Ok((None, ProgressType::FillWithZeros))
                    }
                }
                OutputSource::Stdout(_) | OutputSource::DevNull => {
                    Ok((None, ProgressType::FillWithZeros))
                } // Infinite Capacity
                OutputSource::DevFull => Ok((Some(0), ProgressType::FillWithZeros)), // No capacity
            }
        }
        InputSource::DevUrandom => {
            // Infinite random - base on output capacity
            match output {
                OutputSource::File(_) => {
                    if let Some(path) = output_path {
                        let available = get_available_space(path)?;
                        Ok((available, ProgressType::FillWithRandom))
                    } else {
                        Ok((None, ProgressType::FillWithRandom))
                    }
                }
                OutputSource::Stdout(_) | OutputSource::DevNull => {
                    Ok((None, ProgressType::FillWithRandom))
                } // Infinite capacity
                OutputSource::DevFull => Ok((Some(0), ProgressType::FillWithRandom)), // No capacity
            }
        }
    }
}

/// Represents different types of progress tracking
#[derive(Debug, Clone)]
pub enum ProgressType {
    FileTransfer,
    StreamTransfer,
    FillWithZeros,
    FillWithRandom,
}

/// Get the size of a file in bytes, if there's no file the size will be unknown
pub fn get_file_size(source: &InputSource) -> io::Result<Option<u64>> {
    match source {
        InputSource::File(file) => Ok(Some(file.metadata()?.len())),
        InputSource::Stdin(_) => Ok(None),
        InputSource::DevZero | InputSource::DevUrandom => Ok(None), // Infinite
        InputSource::DevNull => Ok(Some(0)),                        // Always empty
    }
}

/// Checks if an input source is infinite
pub fn is_infinite_source(source: &InputSource) -> bool {
    matches!(source, InputSource::DevZero | InputSource::DevUrandom)
}

/// Creates a progress bar with the specified total size
pub fn create_progress_bar(total_size: Option<u64>, is_infinite: bool) -> ProgressBar {
    let pb;

    match (total_size, is_infinite) {
        (Some(size), false) => {
            pb = ProgressBar::new(size);
            pb.set_style(
                ProgressStyle::with_template(
                    "[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
                )
                .unwrap()
                .progress_chars("=>-"),
            );
        }
        _ => {
            pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template("[{elapsed_precise}] {spinner:.cyan} {bytes} {msg}")
                    .unwrap(),
            );
        }
    }

    pb
}

/// Finish the progress bar with a completion message
pub fn finish_pb_with_message(pb: ProgressBar, total_size: Option<u64>, msg: impl Into<String>) {
    match total_size {
        _ => pb.finish_with_message(msg.into()),
    }
}

/// Create a buffer of the specified size
pub fn create_buffer(size: usize) -> Vec<u8> {
    vec![0; size]
}

/// Copy the data from input to output with progress tracking
pub fn copy_with_progress(
    input: &mut InputSource,
    output: &mut OutputSource,
    buffer_size: usize,
    pb: &ProgressBar,
) -> io::Result<()> {
    let mut buffer = create_buffer(buffer_size);

    loop {
        match input.read(&mut buffer) {
            Ok(0) => break, // Reached the end of the file
            Ok(bytes_read) => {
                output.write_all(&buffer[..bytes_read])?;
                pb.inc(bytes_read as u64);
            }
            Err(e) => {
                eprintln!("Error reading from input file: {e}");
                process::exit(1);
            }
        }
    }

    Ok(())
}
