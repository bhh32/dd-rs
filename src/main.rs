mod cli;

use crate::cli::Cli;
use clap::Parser;
use dd::utils::{copy_with_progress, create_progress_bar, finish_pb_with_message};
use dd::{get_progress_target, open_input_file, open_output_file, validate_special_device_combo};
use std::io;

fn main() -> io::Result<()> {
    let args = Cli::parse();

    validate_special_device_combo(args.input.as_ref(), args.output.as_ref())?;

    let mut input_source = open_input_file(args.input.as_ref())?;
    let mut output_source = open_output_file(args.output.as_ref())?;
    let (target_size, progress_type) =
        get_progress_target(&input_source, &output_source, args.output.as_ref())?;

    let pb = create_progress_bar(target_size, progress_type.clone());

    copy_with_progress(&mut input_source, &mut output_source, args.block_size, &pb)?;
    finish_pb_with_message(pb, progress_type);
    Ok(())
}
