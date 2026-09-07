use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};

pub(crate) fn private_dir(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        anyhow::ensure!(
            !meta.file_type().is_symlink() && meta.is_dir(),
            "Unsafe control directory"
        );
        check_owner(path, &meta)?;
    } else {
        let parent = path.parent().context("Control directory has no parent")?;
        fs::create_dir_all(parent)?;
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        if let Err(error) = builder.create(path) {
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(error.into());
            }
        }
    }
    restrict(path, true)?;
    Ok(())
}

pub(crate) fn private_open(path: &Path, create: bool) -> Result<File> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        anyhow::ensure!(
            !meta.file_type().is_symlink() && meta.is_file(),
            "Unsafe control file"
        );
        check_owner(path, &meta)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(create);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    restrict(path, false)?;
    Ok(file)
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
    let tmp = path.with_extension(format!("{}.tmp", random_token()?));
    let result = (|| {
        let mut file = private_open(&tmp, true)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        // Windows rename cannot replace an existing destination. The directory
        // lock and process state mutex serialize writers; no instructions are
        // reconstructed from an absent state file.
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(&tmp, path)?;
        #[cfg(unix)]
        File::open(path.parent().context("Control file has no parent")?)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
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
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        EqualSid, GetTokenInformation, TokenUser, OWNER_SECURITY_INFORMATION, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    anyhow::ensure!(
        meta.file_attributes() & 0x400 == 0,
        "Control state cannot use a reparse point"
    );
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor = std::ptr::null_mut();
    let mut owner = std::ptr::null_mut();
    let mut token = std::ptr::null_mut();
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
        let result = (|| -> Result<()> {
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
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
            anyhow::ensure!(
                !owner.is_null() && EqualSid(owner, user.User.Sid) != 0,
                "Control state belongs to another Windows user"
            );
            Ok(())
        })();
        if !token.is_null() {
            CloseHandle(token);
        }
        LocalFree(descriptor);
        result
    }
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
