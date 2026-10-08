//! Retain the executable actually running in this host, with OS identity checks.
//! This is an artifact primitive, not Tool registration, ABI/semantic approval,
//! catalogue activation or permission. It does not cover dynamically loaded code.
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
};

use sha2::{Digest, Sha256};

const MAX_IMAGE_BYTES: u64 = 1_073_741_824;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NativeImageError {
    #[error("native executable identity is unsupported on this platform")]
    Unsupported,
    #[error("native executable exceeds artifact capacity")]
    Capacity,
    #[error("native executable retention failed")]
    Storage,
    #[error("native executable identity could not be verified")]
    Identity,
    #[error("native executable verification did not complete")]
    Verification,
    #[error("native executable verifier termination was not acknowledged")]
    Containment,
    #[error("retained native executable checksum changed")]
    Integrity,
}

/// Owns a private artifact copy and its verified content identity. No mutable
/// source path or caller-authored checksum can construct this value. Keeping it
/// alive retains the bytes, not a restart-compatible handler or approval record.
/// Registration must separately bind the actual handler and adapter/ABI to it;
/// incompatible host upgrades still require draining/reconciling open work.
pub struct NativeExecutableImage {
    artifact: Mutex<File>,
    checksum: [u8; 32],
    size: u64,
    // The private directory survives until every owning Arc is released.
    _directory: tempfile::TempDir,
}
impl NativeExecutableImage {
    /// Streaming copy bounds memory and artifact size. On Linux the source is
    /// /proc/self/exe, not a mutable path returned by current_exe. On macOS the
    /// retained signed copy must satisfy the running SecCode's exact cdhash.
    /// An ad-hoc signature establishes code identity, never trusted provenance.
    pub async fn capture(max_bytes: u64) -> Result<Arc<Self>, NativeImageError> {
        if max_bytes == 0 || max_bytes > MAX_IMAGE_BYTES {
            return Err(NativeImageError::Capacity);
        }
        let staged = tokio::task::spawn_blocking(move || stage(max_bytes))
            .await
            .map_err(|_| NativeImageError::Verification)??;
        #[cfg(target_os = "macos")]
        {
            let hash = macos::directory_hash(&staged.path()).await?;
            let path = staged.path();
            tokio::task::spawn_blocking(move || macos::verify_running(&path, &hash))
                .await
                .map_err(|_| NativeImageError::Verification)??;
        }
        let image = Arc::new(staged);
        image.verify().await?;
        Ok(image)
    }

    pub fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    /// Check the retained file descriptor, not the original install pathname.
    /// This is not a substitute for checking selected Tool/ABI/approval records.
    pub async fn verify(self: &Arc<Self>) -> Result<(), NativeImageError> {
        let retained = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut file = retained
                .artifact
                .lock()
                .map_err(|_| NativeImageError::Storage)?;
            file.seek(SeekFrom::Start(0))
                .map_err(|_| NativeImageError::Storage)?;
            let (checksum, size) = hash_file(&mut file, retained.size)?;
            if size != retained.size || checksum != retained.checksum {
                return Err(NativeImageError::Integrity);
            }
            Ok(())
        })
        .await
        .map_err(|_| NativeImageError::Verification)?
    }
    #[cfg(target_os = "macos")]
    fn path(&self) -> std::path::PathBuf {
        self._directory.path().join("executable")
    }
}

fn loaded_file() -> Result<File, NativeImageError> {
    #[cfg(target_os = "linux")]
    {
        File::open("/proc/self/exe").map_err(|_| NativeImageError::Identity)
    }
    #[cfg(target_os = "macos")]
    {
        use security_framework::os::macos::code_signing::{Flags, SecCode};
        let code = SecCode::for_self(Flags::NONE).map_err(|_| NativeImageError::Identity)?;
        let path = code
            .path(Flags::NONE)
            .map_err(|_| NativeImageError::Identity)?
            .to_path()
            .ok_or(NativeImageError::Identity)?;
        File::open(path).map_err(|_| NativeImageError::Identity)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(NativeImageError::Unsupported)
    }
}

fn stage(max_bytes: u64) -> Result<NativeExecutableImage, NativeImageError> {
    let mut original = loaded_file()?;
    let metadata = original.metadata().map_err(|_| NativeImageError::Storage)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(NativeImageError::Capacity);
    }
    let directory = tempfile::Builder::new()
        .prefix("brassclaw-native-")
        .tempdir()
        .map_err(|_| NativeImageError::Storage)?;
    let path = directory.path().join("executable");
    let mut options = File::options();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut copy = options.open(&path).map_err(|_| NativeImageError::Storage)?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 65_536];
    loop {
        let count = original
            .read(&mut buffer)
            .map_err(|_| NativeImageError::Storage)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .filter(|size| *size <= max_bytes)
            .ok_or(NativeImageError::Capacity)?;
        copy.write_all(&buffer[..count])
            .map_err(|_| NativeImageError::Storage)?;
        hash.update(&buffer[..count]);
    }
    if size != metadata.len() {
        return Err(NativeImageError::Identity);
    }
    copy.flush().map_err(|_| NativeImageError::Storage)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        copy.set_permissions(std::fs::Permissions::from_mode(0o400))
            .map_err(|_| NativeImageError::Storage)?;
    }
    drop(copy);
    let artifact = File::open(path).map_err(|_| NativeImageError::Storage)?;
    Ok(NativeExecutableImage {
        artifact: Mutex::new(artifact),
        checksum: hash.finalize().into(),
        size,
        _directory: directory,
    })
}

fn hash_file(file: &mut File, max_bytes: u64) -> Result<([u8; 32], u64), NativeImageError> {
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 65_536];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| NativeImageError::Storage)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .filter(|size| *size <= max_bytes)
            .ok_or(NativeImageError::Integrity)?;
        hash.update(&buffer[..count]);
    }
    Ok((hash.finalize().into(), size))
}

#[cfg(target_os = "macos")]
mod macos {
    use super::NativeImageError;
    use std::{path::Path, process::Stdio, time::Duration};
    use tokio::{io::AsyncReadExt, process::Command};
    const OUTPUT_BYTES: u64 = 65_536;
    const DEADLINE: Duration = Duration::from_secs(5);

    pub(super) async fn directory_hash(path: &Path) -> Result<String, NativeImageError> {
        let arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            "x86_64" => "x86_64",
            _ => return Err(NativeImageError::Unsupported),
        };
        let mut child = Command::new("/usr/bin/codesign")
            .args(["--display", "--verbose=4", "--arch", arch])
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| NativeImageError::Verification)?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or(NativeImageError::Verification)?
            .take(OUTPUT_BYTES + 1);
        let mut stderr = child
            .stderr
            .take()
            .ok_or(NativeImageError::Verification)?
            .take(OUTPUT_BYTES + 1);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result = tokio::time::timeout(DEADLINE, async {
            tokio::try_join!(
                stdout.read_to_end(&mut out),
                stderr.read_to_end(&mut err),
                child.wait()
            )
        })
        .await;
        let Ok(Ok((_, _, status))) = result else {
            child
                .start_kill()
                .map_err(|_| NativeImageError::Containment)?;
            tokio::time::timeout(DEADLINE, child.wait())
                .await
                .map_err(|_| NativeImageError::Containment)?
                .map_err(|_| NativeImageError::Containment)?;
            return Err(NativeImageError::Verification);
        };
        if !status.success() || out.len() as u64 > OUTPUT_BYTES || err.len() as u64 > OUTPUT_BYTES {
            return Err(NativeImageError::Verification);
        }
        let text = std::str::from_utf8(&err).map_err(|_| NativeImageError::Identity)?;
        let hashes: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix("CDHash="))
            .collect();
        if hashes.len() != 1
            || hashes[0].len() != 40
            || !hashes[0].bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(NativeImageError::Identity);
        }
        Ok(hashes[0].to_ascii_lowercase())
    }
    pub(super) fn verify_running(path: &Path, hash: &str) -> Result<(), NativeImageError> {
        use core_foundation::url::CFURL;
        use security_framework::os::macos::code_signing::{Flags, SecRequirement, SecStaticCode};
        // Hash is parsed from bounded OS output; no requirement text supplied by
        // a Recipe, operator record or model is accepted by this interface.
        if hash.len() != 40 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(NativeImageError::Identity);
        }
        let requirement: SecRequirement = format!("cdhash H\"{hash}\"")
            .parse()
            .map_err(|_| NativeImageError::Identity)?;
        let url = CFURL::from_path(path, false).ok_or(NativeImageError::Identity)?;
        SecStaticCode::from_path(&url, Flags::NONE)
            .map_err(|_| NativeImageError::Identity)?
            .check_validity(
                Flags::STRICT_VALIDATE | Flags::NO_NETWORK_ACCESS | Flags::CHECK_ALL_ARCHITECTURES,
                &requirement,
            )
            .map_err(|_| NativeImageError::Identity)?;
        verify_running_code(hash)
    }
    pub(super) fn verify_running_code(hash: &str) -> Result<(), NativeImageError> {
        use security_framework::os::macos::code_signing::{Flags, SecCode, SecRequirement};
        if hash.len() != 40 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(NativeImageError::Identity);
        }
        let requirement: SecRequirement = format!("cdhash H\"{hash}\"")
            .parse()
            .map_err(|_| NativeImageError::Identity)?;
        SecCode::for_self(Flags::NONE)
            .map_err(|_| NativeImageError::Identity)?
            .check_validity(Flags::NONE, &requirement)
            .map_err(|_| NativeImageError::Identity)
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn actual_loaded_executable_is_retained_and_corruption_is_rejected() {
        let image = NativeExecutableImage::capture(MAX_IMAGE_BYTES)
            .await
            .unwrap();
        assert!(image.size() > 0);
        assert_ne!(image.checksum(), [0; 32]);
        image.verify().await.unwrap();
        // Corrupt this private artifact only; the running executable is untouched.
        let path = image._directory.path().join("executable");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut file = File::options().write(true).open(path).unwrap();
        file.write_all(b"BAD!").unwrap();
        file.flush().unwrap();
        assert_eq!(image.verify().await, Err(NativeImageError::Integrity));
    }
    #[tokio::test]
    async fn invalid_capacity_never_returns_a_native_identity() {
        assert!(matches!(
            NativeExecutableImage::capture(0).await,
            Err(NativeImageError::Capacity)
        ));
        assert!(matches!(
            NativeExecutableImage::capture(1).await,
            Err(NativeImageError::Capacity)
        ));
        assert!(matches!(
            NativeExecutableImage::capture(MAX_IMAGE_BYTES + 1).await,
            Err(NativeImageError::Capacity)
        ));
    }
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn another_signed_executable_cannot_impersonate_this_process() {
        let path = std::path::Path::new("/usr/bin/true");
        let hash = macos::directory_hash(path).await.unwrap();
        assert_eq!(
            macos::verify_running_code(&hash),
            Err(NativeImageError::Identity)
        );
    }
}
