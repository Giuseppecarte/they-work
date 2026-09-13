use super::*;
use std::os::windows::fs::OpenOptionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("theywork win 窓 {}", random_token().unwrap()));
        private_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn dacl(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor = std::ptr::null_mut();
    let mut text = std::ptr::null_mut();
    let mut length = 0;
    // SAFETY: all output pointers are valid, the returned allocations are
    // copied before LocalFree, and no Windows allocation escapes this function.
    unsafe {
        assert_eq!(
            GetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descriptor
            ),
            0
        );
        let converted = ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            &mut length,
        );
        if converted == 0 {
            LocalFree(descriptor);
            panic!("Cannot inspect fixture DACL");
        }
        let value = String::from_utf16_lossy(std::slice::from_raw_parts(
            text,
            length.saturating_sub(1) as usize,
        ));
        LocalFree(text.cast());
        LocalFree(descriptor);
        value
    }
}

#[test]
fn replacement_keeps_owner_only_acl_for_state_config_and_endpoint() {
    assert_eq!(
        windows_creation_path(Path::new("local-state")).unwrap(),
        windows_creation_path(Path::new("./local-state")).unwrap(),
        "a relative leaf keeps the current-directory meaning without changing cwd"
    );
    let fixture = Fixture::new();
    // Creation must use TokenUser, including elevated runners whose default
    // TokenOwner is an administrators group. Reopening must not take ownership.
    check_owner(&fixture.0, &fs::symlink_metadata(&fixture.0).unwrap()).unwrap();
    let directory_acl = dacl(&fixture.0);
    assert!(directory_acl.contains("D:P") && directory_acl.contains(";;;OW)"));
    assert_eq!(directory_acl.matches('(').count(), 1, "{directory_acl}");
    private_dir(&fixture.0).unwrap();
    assert_eq!(dacl(&fixture.0), directory_acl);
    let lock_path = fixture.0.join("startup.lock");
    drop(private_open(&lock_path, true).unwrap());
    check_owner(&lock_path, &fs::symlink_metadata(&lock_path).unwrap()).unwrap();
    drop(private_open(&lock_path, true).unwrap());
    for name in ["state.json", "config.json", "endpoint.json"] {
        let path = fixture.0.join(name);
        write_private(&path, b"old").unwrap();
        let before = dacl(&path);
        assert!(
            before.contains("D:P") && before.contains(";;;OW)"),
            "{before}"
        );
        assert_eq!(before.matches('(').count(), 1, "{before}");
        check_owner(&path, &fs::symlink_metadata(&path).unwrap()).unwrap();
        write_private(&path, b"new complete contents").unwrap();
        assert_eq!(read_private(&path).unwrap(), b"new complete contents");
        assert_eq!(dacl(&path), before);
        check_owner(&path, &fs::symlink_metadata(&path).unwrap()).unwrap();
    }
}

#[test]
fn sharing_violation_keeps_original_and_cleans_its_candidate() {
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
    let fixture = Fixture::new();
    let path = fixture.0.join("state.json");
    write_private(&path, b"old complete state").unwrap();
    let held = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&path)
        .unwrap();
    assert!(write_private(&path, b"new complete state").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"old complete state");
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    drop(held);
    write_private(&path, b"new complete state").unwrap();
    assert_eq!(read_private(&path).unwrap(), b"new complete state");
}

#[test]
fn concurrent_readers_never_see_missing_or_partial_state() {
    let fixture = Fixture::new();
    let path = fixture.0.join("state.json");
    let a = vec![b'a'; 4096];
    let b = vec![b'b'; 4096];
    write_private(&path, &a).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let reader_stop = stop.clone();
    let reader_path = path.clone();
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reads = 0;
        while !reader_stop.load(Ordering::SeqCst) {
            let bytes = fs::read(&reader_path).expect("Established state must remain readable");
            assert!(bytes == vec![b'a'; 4096] || bytes == vec![b'b'; 4096]);
            reads += 1;
            if reads == 1 {
                ready_sender.send(()).unwrap();
            }
        }
        reads
    });
    ready_receiver
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    for index in 0..100 {
        write_private(&path, if index % 2 == 0 { &b } else { &a }).unwrap();
    }
    stop.store(true, Ordering::SeqCst);
    assert!(reader.join().unwrap() > 0);
}

#[test]
fn rejects_directory_and_reparse_destination_without_touching_target() {
    let fixture = Fixture::new();
    let target = fixture.0.join("target.json");
    let link = fixture.0.join("state.json");
    write_private(&target, b"untouched").unwrap();
    std::os::windows::fs::symlink_file(&target, &link)
        .expect("Native Windows security gate requires permission to create fixture symlinks");
    assert!(write_private(&link, b"replaced").is_err());
    assert_eq!(fs::read(&target).unwrap(), b"untouched");
    fs::remove_file(link).unwrap();
    fs::create_dir(fixture.0.join("state.json")).unwrap();
    assert!(write_private(&fixture.0.join("state.json"), b"replaced").is_err());
}

#[test]
fn interruption_before_and_after_publication_preserves_complete_state() {
    for phase in ["before-stage", "before-publish", "after-publish"] {
        let fixture = Fixture::new();
        let path = fixture.0.join("state.json");
        write_private(&path, b"old complete state").unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "security::windows_tests::replacement_child",
                "--nocapture",
            ])
            .env("THEYWORK_REPLACE_FIXTURE", &fixture.0)
            .env("THEYWORK_REPLACE_PHASE", phase)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !fixture.0.join("ready").exists() {
            if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
                let _ = child.kill();
                let _ = child.wait();
                panic!("Replacement fixture did not reach {phase}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let bytes = read_private(&path).unwrap();
        assert_eq!(
            bytes,
            if phase == "after-publish" {
                b"new complete state"
            } else {
                b"old complete state"
            }
        );
    }
}

#[test]
fn replacement_child() {
    let Some(root) = std::env::var_os("THEYWORK_REPLACE_FIXTURE") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let phase = std::env::var("THEYWORK_REPLACE_PHASE").unwrap();
    let pause = || -> Result<()> {
        fs::write(root.join("ready"), b"ready")?;
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    };
    if phase == "before-stage" {
        pause().unwrap();
    }
    write_private_with_publisher(
        &root.join("state.json"),
        b"new complete state",
        |_| true,
        |tmp, path| {
            if phase == "before-publish" {
                pause()?;
            }
            publish_private(tmp, path)?;
            pause()
        },
    )
    .unwrap();
}
