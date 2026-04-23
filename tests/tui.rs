#![cfg(all(unix, not(target_os = "macos")))]

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tempfile::TempDir;

#[test]
fn tui_yank_then_quit_exits_when_clipboard_tool_stays_alive() {
    let temp = TempDir::new().unwrap();
    write_project_links(temp.path());
    let clipboard_file = temp.path().join("clipboard.txt");
    let fake_bin = temp.path().join("bin");
    write_fake_wl_copy(&fake_bin);

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_plinks"));
    command.cwd(temp.path());
    command.env("PATH", test_path_with_fake_bin(&fake_bin));
    command.env("WAYLAND_DISPLAY", "plinks-test-wayland");
    command.env("PLINKS_TEST_CLIPBOARD_OUT", &clipboard_file);
    command.env_remove("DISPLAY");

    let mut child = pair.slave.spawn_command(command).unwrap();
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().unwrap();
    let reader_thread = thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = reader.read_to_end(&mut buffer);
        buffer
    });

    let mut writer = pair.master.take_writer().unwrap();
    thread::sleep(Duration::from_millis(500));
    writer.write_all(b"y").unwrap();
    thread::sleep(Duration::from_millis(750));
    writer.write_all(b"q").unwrap();
    drop(writer);

    let status = wait_for_child_exit(&mut child, Duration::from_secs(2));
    if status.is_none() {
        child.kill().unwrap();
        let output = reader_thread.join().unwrap();
        panic!(
            "plinks did not exit after y then q.\nTUI output:\n{}",
            String::from_utf8_lossy(&output)
        );
    }

    assert!(status.unwrap().success());
    assert_eq!(
        fs::read_to_string(&clipboard_file).unwrap(),
        "https://docs.rs"
    );
}

fn write_project_links(project_dir: &Path) {
    fs::write(
        project_dir.join("project-links.toml"),
        r#"version = 1

[links]

[links.docs]
url = "https://docs.rs"
"#,
    )
    .unwrap();
}

fn write_fake_wl_copy(fake_bin: &Path) {
    fs::create_dir_all(fake_bin).unwrap();
    let script = fake_bin.join("wl-copy");
    fs::write(
        &script,
        r#"#!/bin/sh
cat > "$PLINKS_TEST_CLIPBOARD_OUT"
sleep 5
"#,
    )
    .unwrap();

    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).unwrap();
}

fn test_path_with_fake_bin(fake_bin: &Path) -> std::ffi::OsString {
    let mut paths = vec![fake_bin.to_path_buf()];
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    env::join_paths(paths).unwrap()
}

fn wait_for_child_exit(
    child: &mut Box<dyn portable_pty::Child + Send + Sync>,
    timeout: Duration,
) -> Option<portable_pty::ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    }
}
