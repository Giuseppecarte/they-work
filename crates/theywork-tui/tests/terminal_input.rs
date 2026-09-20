#[cfg(unix)]
#[test]
fn mouse_motion_backlog_does_not_delay_clicks_or_keys() {
    let output = std::process::Command::new("python3")
        .args([
            "-c",
            include_str!("terminal_input.py"),
            env!("CARGO_BIN_EXE_they-work"),
        ])
        .output()
        .expect("Python 3 is required for the terminal input fixture");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
