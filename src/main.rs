mod dd;
mod utils;

use crate::dd::Cli;
use crate::utils::{
    copy_with_progress, create_progress_bar, finish_pb_with_message, get_file_size,
    is_infinite_source, open_input_file, open_output_file,
};
use clap::Parser;
use std::io;

fn main() -> io::Result<()> {
    let args = Cli::parse();

    let mut input_source = open_input_file(args.input.as_ref())?;
    let mut output_source = open_output_file(args.output.as_ref())?;
    let total_size = get_file_size(&input_source)?;
    let is_infinite = is_infinite_source(&input_source);
    let pb = create_progress_bar(total_size, is_infinite);

    copy_with_progress(&mut input_source, &mut output_source, args.block_size, &pb)?;
    finish_pb_with_message(pb, total_size, "Copy complete!");
    Ok(())
}
