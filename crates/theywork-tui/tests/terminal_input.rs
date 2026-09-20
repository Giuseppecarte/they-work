#![cfg(unix)]

fn exercise(mode: &str, transport: &str) {
    let output = std::process::Command::new("python3")
        .args([
            "-c",
            include_str!("terminal_input.py"),
            env!("CARGO_BIN_EXE_they-work"),
            mode,
            transport,
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

#[test]
fn mouse_motion_backlog_does_not_delay_clicks_or_keys() {
    exercise("auto", "normal");
}

#[test]
fn explicit_graphics_modes_keep_keyboard_and_mouse_actions() {
    exercise("images", "normal");
    exercise("cells", "normal");
}

#[test]
fn slow_transport_falls_back_and_keeps_navigation_working() {
    exercise("auto", "slow");
}
