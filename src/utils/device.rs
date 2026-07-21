use std::{io, path::PathBuf, process};
use sysinfo::Disks;

#[cfg(unix)]
use block_devs::BlckExt;
#[cfg(unix)]
use std::fs::File;

pub(crate) fn get_available_space(path: &PathBuf) -> io::Result<Option<u64>> {
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

pub(crate) fn check_and_handle_mount(path: &PathBuf, is_output: bool) -> io::Result<()> {
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
