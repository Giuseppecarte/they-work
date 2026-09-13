use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};

#[cfg(all(test, windows))]
#[path = "security_windows_tests.rs"]
mod windows_tests;

pub(crate) fn private_dir(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(meta) => Some(meta),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(meta) = metadata {
        anyhow::ensure!(
            !meta.file_type().is_symlink() && meta.is_dir(),
            "Unsafe control directory"
        );
        check_owner(path, &meta)?;
    } else {
        let parent = path.parent().context("Control directory has no parent")?;
        fs::create_dir_all(parent)?;
        #[cfg(not(windows))]
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        #[cfg(not(windows))]
        let created = builder.create(path).map_err(anyhow::Error::from);
        #[cfg(windows)]
        let created = create_private_windows_dir(path);
        if let Err(error) = created {
            if error
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind)
                != Some(std::io::ErrorKind::AlreadyExists)
            {
                return Err(error);
            }
        }
        // A competing creator must satisfy the same ownership/type checks.
        let meta = fs::symlink_metadata(path)?;
        anyhow::ensure!(
            !meta.file_type().is_symlink() && meta.is_dir(),
            "Unsafe control directory"
        );
        check_owner(path, &meta)?;
    }
    restrict(path, true)?;
    Ok(())
}

pub(crate) fn private_open(path: &Path, create: bool) -> Result<File> {
    private_file_exists(path)?;
    #[cfg(windows)]
    if create {
        match create_private_windows_file(path) {
            Ok(file) => {
                check_owner(path, &file.metadata()?)?;
                // Retain the existing fail-closed ACL-support check even if a
                // filesystem accepted but ignored creation security attributes.
                restrict(path, false)?;
                return Ok(file);
            }
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .map(std::io::Error::kind)
                    == Some(std::io::ErrorKind::AlreadyExists) =>
            {
                // CREATE_NEW never changes an existing owner's descriptor.
                // Validate again if another writer created it after our check.
                private_file_exists(path)?;
            }
            Err(error) => return Err(error),
        }
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(not(windows))]
    options.create(create);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    restrict(path, false)?;
    Ok(file)
}

/// Only NotFound means absence; denied metadata and unsafe files are errors.
pub(crate) fn private_file_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            anyhow::ensure!(
                !meta.file_type().is_symlink() && meta.is_file(),
                "Unsafe control file"
            );
            check_owner(path, &meta)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn read_private_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    if private_file_exists(path)? {
        Ok(Some(read_private(path)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn read_private(path: &Path) -> Result<Vec<u8>> {
    let mut file = private_open(path, false)?;
    let mut data = Vec::new();
    Read::by_ref(&mut file)
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    anyhow::ensure!(
        data.len() <= 16 * 1024 * 1024,
        "Control state exceeds limit"
    );
    Ok(data)
}

pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    write_private_checked(path, bytes, |_| true)
}

pub(crate) fn write_private_checked(
    path: &Path,
    bytes: &[u8],
    valid_destination: impl Fn(&[u8]) -> bool,
) -> Result<()> {
    write_private_impl(path, bytes, valid_destination, publish_private, || Ok(()))
}

#[cfg(test)]
pub(crate) fn write_private_before_sync(
    path: &Path,
    bytes: &[u8],
    before_sync: impl FnOnce() -> Result<()>,
) -> Result<()> {
    write_private_impl(path, bytes, |_| true, publish_private, before_sync)
}

#[cfg(test)]
pub(crate) fn write_private_with_publisher(
    path: &Path,
    bytes: &[u8],
    valid_destination: impl Fn(&[u8]) -> bool,
    publisher: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<()> {
    write_private_impl(path, bytes, valid_destination, publisher, || Ok(()))
}

fn write_private_impl(
    path: &Path,
    bytes: &[u8],
    valid_destination: impl Fn(&[u8]) -> bool,
    publisher: impl FnOnce(&Path, &Path) -> Result<()>,
    before_sync: impl FnOnce() -> Result<()>,
) -> Result<()> {
    private_file_exists(path)?;
    let tmp = path.with_extension(format!("{}.tmp", random_token()?));
    let mut publication_attempted = false;
    let result = (|| {
        let mut file = private_open(&tmp, true)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        publication_attempted = true;
        publisher(&tmp, path)?;
        before_sync()?;
        #[cfg(unix)]
        File::open(path.parent().context("Control file has no parent")?)?.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        if !publication_attempted
            || read_private(path)
                .map(|bytes| valid_destination(&bytes))
                .unwrap_or(false)
        {
            let _ = fs::remove_file(tmp);
            return Err(error);
        }
        let candidate = match fs::symlink_metadata(&tmp) {
            Ok(_) => format!("staged candidate retained at {}", tmp.display()),
            Err(problem) if problem.kind() == std::io::ErrorKind::NotFound => {
                "no staged candidate remains".into()
            }
            Err(_) => format!(
                "staged candidate could not be inspected at {}; preserve it if present",
                tmp.display()
            ),
        };
        return Err(error.context(format!(
            "Replacement could not be verified; {candidate}; storage recovery is required; nothing will be replayed"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn publish_private(tmp: &Path, path: &Path) -> Result<()> {
    fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(windows)]
fn publish_private(tmp: &Path, path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let wide = |value: &Path| -> Result<Vec<u16>> {
        let mut encoded: Vec<u16> = value.as_os_str().encode_wide().collect();
        anyhow::ensure!(
            !encoded.contains(&0),
            "Control path contains a null character"
        );
        encoded.push(0);
        Ok(encoded)
    };
    // std canonicalization supplies the extended Windows prefix when needed;
    // derive the destination from that same parent without resolving its name.
    let source_path = fs::canonicalize(tmp)?;
    let destination_path = source_path
        .parent()
        .context("Control temporary file has no parent")?
        .join(path.file_name().context("Control file has no name")?);
    let source = wide(&source_path)?;
    let destination = wide(&destination_path)?;
    // SAFETY: both terminated buffers live for the call. The temporary file is
    // a sibling on the same volume; no copy/delete or delayed move is enabled.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(unix)]
fn check_owner(_path: &Path, meta: &fs::Metadata) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no arguments or memory effects.
    anyhow::ensure!(
        meta.uid() == unsafe { libc::geteuid() },
        "Control state belongs to another user"
    );
    Ok(())
}

#[cfg(windows)]
fn check_owner(path: &Path, meta: &fs::Metadata) -> Result<()> {
    use std::os::windows::{ffi::OsStrExt, fs::MetadataExt};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{EqualSid, OWNER_SECURITY_INFORMATION};
    anyhow::ensure!(
        meta.file_attributes() & 0x400 == 0,
        "Control state cannot use a reparse point"
    );
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor = std::ptr::null_mut();
    let mut owner = std::ptr::null_mut();
    // SAFETY: owned buffers meet the Windows APIs' alignment/size contracts.
    // Every acquired native allocation/handle is released on all paths.
    unsafe {
        let status = GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut descriptor,
        );
        if status != 0 {
            return Err(std::io::Error::from_raw_os_error(status as i32).into());
        }
        let result = with_windows_user(|user| {
            anyhow::ensure!(
                !owner.is_null() && EqualSid(owner, user) != 0,
                "Control state belongs to another Windows user"
            );
            Ok(())
        });
        LocalFree(descriptor);
        result
    }
}

#[cfg(windows)]
fn with_windows_user<T>(
    action: impl FnOnce(windows_sys::Win32::Security::PSID) -> Result<T>,
) -> Result<T> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    let mut token = std::ptr::null_mut();
    // SAFETY: the aligned token buffer owns the SID throughout the callback,
    // and the acquired process-token handle is closed on every result path.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let result = (|| {
            let mut needed = 0;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
            anyhow::ensure!(
                needed >= std::mem::size_of::<TOKEN_USER>() as u32,
                "Could not read Windows user identity"
            );
            let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let user = &*(buffer.as_ptr().cast::<TOKEN_USER>());
            action(user.User.Sid)
        })();
        CloseHandle(token);
        result
    }
}

#[cfg(windows)]
fn with_private_windows_attributes<T>(
    action: impl FnOnce(&windows_sys::Win32::Security::SECURITY_ATTRIBUTES) -> Result<T>,
) -> Result<T> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    with_windows_user(|user| {
        let mut sid = std::ptr::null_mut();
        // SAFETY: user lives through this callback; both Windows allocations
        // are copied/used while live and released before returning.
        unsafe {
            if ConvertSidToStringSidW(user, &mut sid) == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut length = 0;
            while *sid.add(length) != 0 {
                length += 1;
            }
            let owner = String::from_utf16_lossy(std::slice::from_raw_parts(sid, length));
            LocalFree(sid.cast());
            // TokenOwner can be an administrators group even though TokenUser
            // is one person. Set that exact user at creation, never take over an
            // existing object. The protected owner-only DACL stays unchanged.
            let sddl: Vec<u16> = format!("O:{owner}D:P(A;OICI;FA;;;OW)\0")
                .encode_utf16()
                .collect();
            let mut descriptor = std::ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            };
            let result = action(&attributes);
            LocalFree(descriptor);
            result
        }
    })
}

#[cfg(windows)]
fn windows_creation_path(path: &Path) -> Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    // Canonicalize only the existing parent: support long paths while leaving
    // the final component for CREATE_NEW/CreateDirectory to check atomically.
    let parent = path.parent().context("Control path has no parent")?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let parent = fs::canonicalize(parent)?;
    let name = path.file_name().context("Control path has no name")?;
    let mut wide: Vec<u16> = parent.join(name).as_os_str().encode_wide().collect();
    anyhow::ensure!(!wide.contains(&0), "Control path contains a null character");
    wide.push(0);
    Ok(wide)
}

#[cfg(windows)]
fn create_private_windows_dir(path: &Path) -> Result<()> {
    use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
    let path = windows_creation_path(path)?;
    with_private_windows_attributes(|attributes| {
        // SAFETY: both path and security descriptor live through the call.
        if unsafe { CreateDirectoryW(path.as_ptr(), attributes) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    })
}

#[cfg(windows)]
fn create_private_windows_file(path: &Path) -> Result<File> {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE,
    };
    let path = windows_creation_path(path)?;
    with_private_windows_attributes(|attributes| {
        // SAFETY: terminated path and descriptor remain live. File takes sole
        // ownership of the returned non-inheritable handle on success.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                attributes,
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(unsafe { File::from_raw_handle(handle) })
    })
}

#[cfg(not(any(unix, windows)))]
fn check_owner(_path: &Path, _meta: &fs::Metadata) -> Result<()> {
    bail!("Owner identity unavailable")
}

#[cfg(unix)]
fn restrict(path: &Path, directory: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
    )?;
    Ok(())
}

#[cfg(windows)]
fn restrict(path: &Path, _directory: bool) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::{
        SetFileSecurityW, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    };
    // Protected DACL: only the file owner. Children inherit owner-only access.
    let sddl: Vec<u16> = "D:P(A;OICI;FA;;;OW)\0".encode_utf16().collect();
    let mut descriptor = std::ptr::null_mut();
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: terminated strings and output pointer live through each call;
    // the descriptor is released exactly once after SetFileSecurityW.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let ok = SetFileSecurityW(
            path.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        let error = std::io::Error::last_os_error();
        LocalFree(descriptor);
        if ok == 0 {
            return Err(error.into());
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn restrict(_path: &Path, _directory: bool) -> Result<()> {
    bail!("Owner-only storage unavailable on this platform")
}

pub(crate) fn random_token() -> Result<String> {
    let mut bytes = [0u8; 32];
    #[cfg(unix)]
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        // SAFETY: valid mutable 32-byte buffer, system RNG with no algorithm handle.
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        anyhow::ensure!(status >= 0, "Windows random source failed");
    }
    #[cfg(not(any(unix, windows)))]
    bail!("Secure random source unavailable");
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn token_matches(left: &str, right: &str) -> bool {
    if left.len() != 64 || right.len() != 64 {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

pub(crate) fn read_line_limited(reader: &mut impl std::io::BufRead) -> Result<Option<String>> {
    read_line_limit(reader, 1024 * 1024)
}

pub(crate) fn read_line_limit(
    reader: &mut impl std::io::BufRead,
    limit: usize,
) -> Result<Option<String>> {
    let mut bytes = Vec::new();
    let size = reader
        .take(limit as u64 + 1)
        .read_until(b'\n', &mut bytes)?;
    if size == 0 {
        return Ok(None);
    }
    if size > limit || !bytes.ends_with(b"\n") {
        bail!("Oversized or incomplete protocol message");
    }
    Ok(Some(String::from_utf8(bytes)?))
}
