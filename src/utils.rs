use crossbeam::channel;
use indicatif::{ProgressBar, ProgressStyle};
use rand::{RngCore, rng};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write, stdin, stdout},
    path::PathBuf,
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};
use sysinfo::Disks;

#[cfg(unix)]
use block_devs::BlckExt;

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
                _ => Ok(OutputSource::File(
                    OpenOptions::new().write(true).create(true).open(path)?,
                )),
            }
        }
        None => Ok(OutputSource::Stdout(stdout())),
    }
}

fn get_available_space(path: &PathBuf) -> io::Result<Option<u64>> {
    let disks = Disks::new_with_refreshed_list();

    for disk in &disks {
        let disk_name = disk.name().to_string_lossy();
        if path.to_string_lossy() == disk_name {
            return Ok(Some(disk.total_space()));
        }

        #[cfg(not(windows))]
        {
            if let Some(device_name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(disk_device_name) = std::path::Path::new(&disk_name.to_string())
                    .file_name()
                    .and_then(|n| n.to_str())
                {
                    if device_name == disk_device_name {
                        return Ok(Some(disk.total_space()));
                    }
                }
            }
        }
    }

    get_block_device_size(path)
}

fn get_block_device_size(path: &PathBuf) -> io::Result<Option<u64>> {
    #[cfg(unix)]
    {
        get_unix_block_device_size(path)
    }

    #[cfg(windows)]
    {
        get_windows_block_device_size(path)
    }

    #[cfg(not(any(unix, windows)))]
    {
        Ok(None)
    }
}

#[cfg(unix)]
fn get_unix_block_device_size(path: &PathBuf) -> io::Result<Option<u64>> {
    match File::open(path) {
        Ok(file) => match file.get_block_device_size() {
            Ok(size) => Ok(Some(size)),
            Err(_) => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

#[cfg(windows)]
fn get_windows_block_device_size(path: &PathBuf) -> io::Result<Option<u64>> {
    use std::ffi::CString;
    use std::mem;
    use std::ptr;
    use winapi::um::fileapi::{CreateFileA, OPEN_EXISTING};
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::ioapiset::DeviceIoControl;
    use winapi::um::winioctl::IOCTL_DISK_GET_DRIVE_GEOMETRY_EX;
    use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ};

    let path_str = path.to_string_lossy();

    let device_path = if path_str.starts_with("\\\\.\\") {
        path_str.to_string()
    } else if path_str.starts_with("PhysicalDrive") {
        format!("\\\\.\\{}", path_str)
    } else if let Some(drive_letter) = path_str.chars().next() {
        if path_str.len() == 2 && path_str.ends_with(':') {
            format!("\\\\.\\{}:", drive_letter)
        } else {
            return Ok(None);
        }
    } else {
        return Ok(None);
    };

    let c_path = match CString::new(device_path) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    let handle = CreateFileA(
        c_path.as_ptr(),
        GENERIC_READ,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        ptr::null_mut(),
        OPEN_EXISTING,
        0,
        ptr::null_mut(),
    );

    if handle == INVALID_HANDLE_VALUE {
        return Ok(None);
    }

    #[repr(C)]
    struct DiskGeometryEx {
        geometry: [u8; 32],
        disk_size: u64,
        data: [u8; 8],
    }

    let mut geometry = DiskGeometryEx {
        geometry: [0; 32],
        disk_size: 0,
        data: [0; 8],
    };
    let mut bytes_returned = 0u32;

    let success = DeviceIoControl(
        handle,
        IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
        ptr::null_mut(),
        0,
        &mut geometry as *mut _ as *mut _,
        mem::size_of::<DiskGeometryEx>() as u32,
        &mut bytes_returned,
        ptr::null_mut(),
    );

    CloseHandle(handle);

    if success != 0 {
        Ok(Some(geometry.disk_size))
    } else {
        Ok(None)
    }
}

fn is_system_drive(path: &PathBuf) -> bool {
    let disks = Disks::new_with_refreshed_list();
    let path_str = path.to_string_lossy();

    for disk in &disks {
        let mount_point = disk.mount_point().to_string_lossy();

        #[cfg(unix)]
        {
            if mount_point == "/" || mount_point == "/boot" || mount_point == "/usr" {
                if path_str.contains(disk.name().to_string_lossy().as_ref()) {
                    return true;
                }
            }
        }

        #[cfg(windows)]
        {
            if mount_point.starts_with("C:") {
                if path_str.contains(disk.name().to_string_lossy().as_ref()) {
                    return true;
                }
            }
        }
    }

    false
}

fn check_and_handle_mount(path: &PathBuf, is_output: bool) -> io::Result<()> {
    if !is_output {
        return Ok(());
    }

    if is_system_drive(path) {
        return Ok(());
    }

    let disks = Disks::new_with_refreshed_list();
    let path_str = path.to_string_lossy();

    for disk in &disks {
        let disk_name = disk.name().to_string_lossy();
        let mount_point = disk.mount_point();

        let is_mounted = path_str == disk_name || {
            #[cfg(not(windows))]
            {
                if let Some(device_name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(disk_device_name) = std::path::Path::new(&disk_name.to_string())
                        .file_name()
                        .and_then(|n| n.to_str())
                    {
                        device_name == disk_device_name
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            #[cfg(windows)]
            {
                false
            }
        };

        if is_mounted {
            eprintln!(
                "Device {} is mounted at {}",
                path_str,
                mount_point.display()
            );
            eprintln!("Unmounting for safe operation...");

            #[cfg(unix)]
            {
                let umount_result = std::process::Command::new("umount")
                    .arg(mount_point)
                    .output();

                match umount_result {
                    Ok(output) if output.status.success() => {
                        eprintln!("Unmounted successfully");
                        return Ok(());
                    }
                    _ => {
                        eprintln!("Failed to unmount. Please unmount manually.");
                        process::exit(1);
                    }
                }
            }

            #[cfg(windows)]
            {
                eprintln!("Please eject/unmount the device manually and try again.");
                process::exit(1);
            }
        }
    }

    Ok(())
}

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

/// Create a buffer of the specified size
pub fn create_buffer(size: usize) -> Vec<u8> {
    vec![0; size]
}

struct DataBuffer {
    data: Vec<u8>,
    bytes_used: usize,
}

impl DataBuffer {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
            bytes_used: 0,
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.data[..self.bytes_used]
    }
}

pub struct ThreadedCopyConfig {
    pub buffer_size: usize,
    pub buffer_count: usize,
}

pub fn copy_with_callback<F>(
    mut input: InputSource,
    mut output: OutputSource,
    config: ThreadedCopyConfig,
    callback: F,
) -> io::Result<()>
where
    F: Fn(u64) + Send + Sync,
{
    let bytes_processed = Arc::new(AtomicU64::new(0));
    let result = Arc::new(std::sync::Mutex::new(Ok(())));

    let (empty_tx, empty_rx) = channel::bounded(config.buffer_count);
    let (full_tx, full_rx) = channel::bounded(config.buffer_count);

    for _ in 0..config.buffer_count {
        empty_tx.send(DataBuffer::new(config.buffer_size)).unwrap();
    }

    thread::scope(|scope| {
        scope.spawn(|| {
            let read_result = (|| -> io::Result<()> {
                loop {
                    let mut buffer = match empty_rx.recv() {
                        Ok(buf) => buf,
                        Err(_) => break,
                    };

                    match input.read(&mut buffer.data) {
                        Ok(0) => {
                            drop(full_tx);
                            break;
                        }
                        Ok(bytes_read) => {
                            buffer.bytes_used = bytes_read;
                            if full_tx.send(buffer).is_err() {
                                break;
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            })();

            if let Err(e) = read_result {
                *result.lock().unwrap() = Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Read error: {e}"),
                ));
            }
        });

        scope.spawn(|| {
            let write_result = (|| -> io::Result<()> {
                loop {
                    let buffer = match full_rx.recv() {
                        Ok(buf) => buf,
                        Err(_) => break,
                    };

                    output.write_all(buffer.as_slice())?;
                    output.flush()?;

                    bytes_processed.fetch_add(buffer.bytes_used as u64, Ordering::Relaxed);

                    if empty_tx.send(DataBuffer::new(config.buffer_size)).is_err() {
                        break;
                    }
                }
                Ok(())
            })();

            if let Err(e) = write_result {
                *result.lock().unwrap() = Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Write error: {e}"),
                ));
            }
        });

        scope.spawn(|| {
            let mut last_bytes = 0u64;
            loop {
                thread::sleep(Duration::from_millis(100));

                let current_bytes = bytes_processed.load(Ordering::Relaxed);
                let delta = current_bytes - last_bytes;

                if delta > 0 {
                    callback(delta);
                    last_bytes = current_bytes;
                }

                if empty_rx.is_empty() && full_rx.is_empty() {
                    let final_bytes = bytes_processed.load(Ordering::Relaxed);
                    let final_delta = final_bytes - last_bytes;
                    if final_delta > 0 {
                        callback(final_delta);
                    }
                    break;
                }
            }
        });
    });

    result
        .lock()
        .unwrap()
        .as_ref()
        .map_err(|e| io::Error::new(e.kind(), e.to_string()))?;

    Ok(())
}

pub fn copy_with_progress(
    input: InputSource,
    output: OutputSource,
    config: ThreadedCopyConfig,
    pb: &ProgressBar,
) -> io::Result<()> {
    copy_with_callback(input, output, config, |bytes| {
        pb.inc(bytes);
    })
}
