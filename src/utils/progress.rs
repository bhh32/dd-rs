use indicatif::{ProgressBar, ProgressStyle};
use std::{io, path::PathBuf};

use super::device::{check_and_handle_mount, get_available_space};
use super::{InputSource, OutputSource};

/// Get the progress target size based on input/output combination
pub fn get_progress_target(
    input: &InputSource,
    output: &OutputSource,
    output_path: Option<&PathBuf>,
) -> io::Result<(Option<u64>, ProgressType)> {
    match input {
        InputSource::File(file) => {
            if let Some(path) = output_path {
                check_and_handle_mount(path, true)?;
            }

            let size = file.metadata()?.len();
            Ok((Some(size), ProgressType::FileTransfer))
        }
        InputSource::Stdin(_) => {
            // Stdin - unknown size
            Ok((None, ProgressType::StreamTransfer))
        }
        InputSource::Http(resp) => Ok((resp.content_length(), ProgressType::FileTransfer)),
        InputSource::DevNull => {
            // /dev/null input - immediate EOF
            Ok((Some(0), ProgressType::FileTransfer))
        }
        InputSource::DevZero => {
            if let Some(path) = output_path {
                check_and_handle_mount(path, true)?;
            }

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
            if let Some(path) = output_path {
                check_and_handle_mount(path, true)?;
            }

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

/// Creates a progress bar with the specified total size
pub fn create_progress_bar(total_size: Option<u64>, progress_type: ProgressType) -> ProgressBar {
    let pb;

    match (total_size, &progress_type) {
        (Some(size), ProgressType::FileTransfer)
        | (Some(size), ProgressType::FillWithZeros)
        | (Some(size), ProgressType::FillWithRandom) => {
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
            let _message = match progress_type {
                ProgressType::FileTransfer => "Copying",
                ProgressType::StreamTransfer => "Streaming",
                ProgressType::FillWithZeros => "Filling with zeros",
                ProgressType::FillWithRandom => "Filling with random data",
            };

            pb.set_style(
                ProgressStyle::with_template(
                    "[{elapsed_precise}] {spinner:.cyan} {bytes} {message}",
                )
                .unwrap(),
            );
        }
    }

    pb
}

/// Finish the progress bar with a completion message
pub fn finish_pb_with_message(pb: ProgressBar, progress_type: ProgressType) {
    let msg = match progress_type {
        ProgressType::FileTransfer => "File copy complete!",
        ProgressType::StreamTransfer => "Stream complete!",
        ProgressType::FillWithZeros => "Data has been overwritten with zeros!",
        ProgressType::FillWithRandom => "Data has been overwritten with random data!",
    };

    pb.finish_with_message(msg);
}
