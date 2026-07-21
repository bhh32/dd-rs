use std::{io, path::PathBuf};

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
