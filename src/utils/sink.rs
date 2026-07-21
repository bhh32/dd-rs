use std::{
    fs::{File, OpenOptions},
    io::{self, Write, stdout},
    path::PathBuf,
};

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

/// Opens an output file for writing
pub fn open_output_file(path: Option<&PathBuf>) -> io::Result<OutputSource> {
    match path {
        Some(path) => {
            let path_str = path.to_string_lossy();
            match path_str.as_ref() {
                "/dev/null" => Ok(OutputSource::DevNull),
                "/dev/full" => Ok(OutputSource::DevFull),
                _ => Ok(OutputSource::File(
                    OpenOptions::new().write(true).create(true).open(path)?,
                )),
            }
        }
        None => Ok(OutputSource::Stdout(stdout())),
    }
}
