use rand::{RngCore, rng};
use reqwest::blocking;
use std::{
    fs::File,
    io::{self, Read, stdin},
    path::PathBuf,
};

/// Represents either a file or stdin for input
pub enum InputSource {
    File(File),
    Stdin(io::Stdin),
    Http(Box<blocking::Response>),
    DevZero,
    DevNull,
    DevUrandom,
}

impl Read for InputSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            InputSource::File(file) => file.read(buf),
            InputSource::Stdin(stdin) => stdin.read(buf),
            InputSource::Http(resp) => resp.read(buf),
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
