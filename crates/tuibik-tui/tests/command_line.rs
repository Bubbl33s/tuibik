use std::process::Command;

fn tuibik() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tuibik"))
}

#[test]
fn long_help_is_non_interactive() {
    let output = tuibik().arg("--help").output().unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: tuibik"));
    assert!(output.stderr.is_empty());
    assert_no_terminal_control_sequences(&output.stdout);
}

#[test]
fn short_help_is_non_interactive() {
    let output = tuibik().arg("-h").output().unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: tuibik"));
    assert!(output.stderr.is_empty());
    assert_no_terminal_control_sequences(&output.stdout);
}

#[test]
fn long_version_is_non_interactive() {
    let output = tuibik().arg("--version").output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("tuibik {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
    assert_no_terminal_control_sequences(&output.stdout);
}

#[test]
fn short_version_is_non_interactive() {
    let output = tuibik().arg("-V").output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("tuibik {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
    assert_no_terminal_control_sequences(&output.stdout);
}

#[test]
fn unsupported_argument_fails_without_initializing_the_tui() {
    let output = tuibik().arg("--unknown-option").output().unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Try 'tuibik --help'"));
    assert_no_terminal_control_sequences(&output.stderr);
}

fn assert_no_terminal_control_sequences(output: &[u8]) {
    assert!(
        !output.contains(&0x1b),
        "non-interactive output must not enter raw or alternate-screen mode"
    );
}
