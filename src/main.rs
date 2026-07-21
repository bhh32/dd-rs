mod cli;

use crate::cli::Cli;
use clap::Parser;
use dd::utils::{copy_with_progress, create_progress_bar, finish_pb_with_message};
use dd::{
    InputSource, OutputSource, ProgressType, get_progress_target, open_input_file, open_output_file,
    validate_special_device_combo,
};
use reqwest::blocking;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
use tempfile::NamedTempFile;

fn main() -> io::Result<()> {
    let args = Cli::parse();

    let _tmp;
    let input_path = match args.input.as_ref() {
        Some(path) if is_url(path) => {
            let tmp = download_to_temp(&path.to_string_lossy(), args.block_size)?;

            if let Some(expected) = &args.sha256 {
                verify_sha256(tmp.path(), expected)?;
                eprintln!("checksum OK");
            }

            let path = tmp.path().to_path_buf();
            _tmp = Some(tmp);
            Some(path)
        }
        other => {
            _tmp = None;
            other.cloned()
        }
    };

    validate_special_device_combo(input_path.as_ref(), args.output.as_ref())?;

    let mut input_source = open_input_file(input_path.as_ref())?;
    let mut output_source = open_output_file(args.output.as_ref())?;

    let (target_size, progress_type) =
        get_progress_target(&input_source, &output_source, args.output.as_ref())?;

    let pb = create_progress_bar(target_size, progress_type.clone());

    copy_with_progress(&mut input_source, &mut output_source, args.block_size, &pb)?;
    finish_pb_with_message(pb, progress_type);
    Ok(())
}

fn is_url(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    path_str.starts_with("http://") || path_str.starts_with("https://")
}

fn download_to_temp(url: &str, block_size: usize) -> io::Result<NamedTempFile> {
    let resp = blocking::get(url)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("HTTP {}", resp.status()),
        ));
    }

    let mut input = InputSource::Http(Box::new(resp));
    let tmp = NamedTempFile::new()?;
    let mut output = OutputSource::File(tmp.reopen()?);

    let (size, _) = get_progress_target(&input, &output, None)?;
    let pb = create_progress_bar(size, ProgressType::FileTransfer);

    copy_with_progress(&mut input, &mut output, block_size, &pb)?;
    pb.finish_and_clear();
    Ok(tmp)
}

fn verify_sha256(path: &Path, expected: &str) -> io::Result<()> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];

    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    let digest = hasher.finalize();
    let got = digest.iter().map(|b| format!("{b:02x}")).collect::<String>();

    if !got.eq_ignore_ascii_case(expected.trim()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("checksum mismatch:\nexpected {expected}\ngot {got}"),
        ));
    }

    Ok(())
}
