//! Resolve the actual main image mapping, then fence its file against writes
//! and renames before copying. No request or mutable current_exe path is used.
use super::NativeImageError;
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    os::windows::{ffi::OsStringExt, fs::OpenOptionsExt},
    ptr,
};
use windows_sys::Win32::{
    Foundation::HMODULE,
    Storage::FileSystem::FILE_SHARE_READ,
    System::{
        LibraryLoader::{GET_MODULE_HANDLE_EX_FLAG_PIN, GetModuleHandleExW},
        ProcessStatus::K32GetMappedFileNameW,
        Threading::GetCurrentProcess,
    },
};

fn main_image() -> Result<HMODULE, NativeImageError> {
    let mut module = ptr::null_mut();
    // SAFETY: null selects this process's executable, and the output points to
    // a live HMODULE slot. PIN retains the image until process shutdown; this
    // borrowed pinned module must not be passed to CloseHandle/FreeLibrary.
    if unsafe { GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_PIN, ptr::null(), &mut module) } == 0
        || module.is_null()
    {
        return Err(NativeImageError::Identity);
    }
    Ok(module)
}

fn mapped_name(module: HMODULE) -> Result<Vec<u16>, NativeImageError> {
    // Initial buffer size only, not a filename cap. Grow on the documented
    // truncation result, rejecting allocation/representation failure explicitly.
    let mut capacity = 256usize;
    loop {
        let length = u32::try_from(capacity).map_err(|_| NativeImageError::Capacity)?;
        let mut name = Vec::new();
        name.try_reserve_exact(capacity)
            .map_err(|_| NativeImageError::Capacity)?;
        name.resize(capacity, 0);
        // SAFETY: module is a live pinned main-image address; the current
        // process pseudo-handle is always valid, and name has length writable
        // UTF-16 elements. Neither borrowed OS handle is closed here.
        let count = unsafe {
            K32GetMappedFileNameW(
                GetCurrentProcess(),
                module.cast(),
                name.as_mut_ptr(),
                length,
            )
        };
        if count == 0 {
            return Err(NativeImageError::Identity);
        }
        if count >= length {
            capacity = capacity.checked_mul(2).ok_or(NativeImageError::Capacity)?;
            continue;
        }
        name.truncate(count as usize);
        if name.contains(&0) || !name.starts_with(&"\\Device\\".encode_utf16().collect::<Vec<_>>())
        {
            return Err(NativeImageError::Identity);
        }
        return Ok(name);
    }
}

fn device_path(name: &[u16]) -> OsString {
    let mut path = OsString::from(r"\\?\GLOBALROOT");
    path.push(OsString::from_wide(name));
    path
}

pub(super) fn loaded_file() -> Result<File, NativeImageError> {
    let module = main_image()?;
    let before = mapped_name(module)?;
    // FILE_SHARE_READ permits concurrent readers but denies write/delete
    // sharing. Holding this handle fences in-place mutation and replacement
    // while the caller stages and hashes the image. Never fall back to a path
    // returned by current_exe/GetModuleFileName on failure.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(device_path(&before))
        .map_err(|_| NativeImageError::Identity)?;
    if before != mapped_name(module)? {
        return Err(NativeImageError::Identity);
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    #[test]
    fn actual_mapped_image_file_is_fenced_and_has_pe_header() {
        let mut image = loaded_file().unwrap();
        let path = device_path(&mapped_name(main_image().unwrap()).unwrap());
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        let mut header = [0u8; 2];
        image.read_exact(&mut header).unwrap();
        assert_eq!(header, *b"MZ");
    }

    #[test]
    fn unicode_device_path_keeps_the_complete_name() {
        let name: Vec<_> = "\\Device\\HarddiskVolume1\\native Ü folder\\app.exe"
            .encode_utf16()
            .collect();
        assert_eq!(
            device_path(&name),
            OsString::from(r"\\?\GLOBALROOT\Device\HarddiskVolume1\native Ü folder\app.exe")
        );
    }

    // This helper runs as a real copied Windows test executable. The parent
    // instructs only this cfg(test) entry; there is no production override.
    #[test]
    fn mapped_image_rename_and_replacement_child() {
        let Some(directory) = std::env::var_os("BRASSCLAW_NATIVE_IMAGE_RENAME_TEST_DIRECTORY")
        else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        let original = std::env::current_exe().unwrap();
        let mut image = loaded_file().unwrap();
        let mut baseline = Vec::new();
        image.read_to_end(&mut baseline).unwrap();
        drop(image);
        let renamed = directory.join("renamed source.exe");
        std::fs::rename(&original, &renamed).unwrap();
        std::fs::write(&original, b"a replacement is not the running image").unwrap();
        let mut retained = loaded_file().unwrap();
        let mut actual = Vec::new();
        retained.read_to_end(&mut actual).unwrap();
        assert_eq!(Sha256::digest(actual), Sha256::digest(baseline));
    }

    #[test]
    fn actual_mapping_survives_rename_and_replacement_in_unicode_directory() {
        let directory = tempfile::Builder::new()
            .prefix("native Ü identity ")
            .tempdir()
            .unwrap();
        let child = directory.path().join("running image.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &child).unwrap();
        let output = std::process::Command::new(&child)
            .args([
                "native_image::windows::tests::mapped_image_rename_and_replacement_child",
                "--exact",
                "--nocapture",
            ])
            .env(
                "BRASSCLAW_NATIVE_IMAGE_RENAME_TEST_DIRECTORY",
                directory.path(),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
}
