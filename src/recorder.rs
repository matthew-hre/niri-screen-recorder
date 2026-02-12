use chrono::Local;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::path::PathBuf;
use std::process::{Child, Command};

fn ensure_screencasts_dir() -> Result<PathBuf, String> {
    let dir = if let Ok(custom) = std::env::var("NIRI_SCREEN_RECORDER_OUTPUT_DIR") {
        PathBuf::from(custom)
    } else {
        let home = dirs::video_dir()
            .or_else(|| dirs::home_dir())
            .ok_or("Cannot find home directory")?;
        home.join("Screencasts")
    };

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    Ok(dir)
}

fn generate_filename() -> Result<String, String> {
    let dir = ensure_screencasts_dir()?;
    let container =
        std::env::var("NIRI_SCREEN_RECORDER_CONTAINER").unwrap_or_else(|_| "mp4".to_string());
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let filename = format!("screen-record-{}.{}", timestamp, container);
    Ok(dir.join(filename).to_string_lossy().to_string())
}

fn detect_cursor_theme() -> Option<String> {
    let config_path = dirs::config_dir()?.join("niri/config.kdl");
    let content = std::fs::read_to_string(config_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("xcursor-theme") {
            return trimmed
                .strip_prefix("xcursor-theme")
                .and_then(|s| s.trim().strip_prefix('"'))
                .and_then(|s| s.strip_suffix('"'))
                .map(|s| s.to_string());
        }
    }
    None
}

/// Use slurp to select a screen region (blocking)
/// Returns a string in the format "WxH+X+Y" for gpu-screen-recorder
pub fn select_region() -> Result<String, String> {
    let mut cmd = Command::new("slurp");
    cmd.arg("-f").arg("%wx%h+%x+%y");

    if std::env::var("XCURSOR_THEME").is_err() {
        if let Some(theme) = detect_cursor_theme() {
            cmd.env("XCURSOR_THEME", theme);
        }
    }
    if std::env::var("XCURSOR_SIZE").is_err() {
        cmd.env("XCURSOR_SIZE", "24");
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run slurp: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Region selection cancelled: {}", stderr.trim()));
    }

    let region = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if region.is_empty() {
        return Err("No region selected".to_string());
    }

    Ok(region)
}

pub fn start_recording(region: &str) -> Result<(Child, String), String> {
    let output_file = generate_filename()?;
    let fps = std::env::var("NIRI_SCREEN_RECORDER_FPS").unwrap_or_else(|_| "60".to_string());
    let container =
        std::env::var("NIRI_SCREEN_RECORDER_CONTAINER").unwrap_or_else(|_| "mp4".to_string());

    let mut cmd = Command::new("gpu-screen-recorder");
    cmd.arg("-w")
        .arg(region)
        .arg("-c")
        .arg(&container)
        .arg("-f")
        .arg(&fps)
        .arg("-o")
        .arg(&output_file);

    if let Ok(codec) = std::env::var("NIRI_SCREEN_RECORDER_CODEC") {
        cmd.arg("-k").arg(&codec);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start gpu-screen-recorder: {}", e))?;

    Ok((child, output_file))
}

/// Stop the recording by sending SIGINT for clean shutdown
pub fn stop_recording(child: &mut Child) -> Result<(), String> {
    let pid = Pid::from_raw(child.id() as i32);

    // Send SIGINT for graceful shutdown (lets gpu-screen-recorder finalize the file)
    kill(pid, Signal::SIGINT).map_err(|e| format!("Failed to send SIGINT: {}", e))?;

    // Wait for the process to actually exit
    child
        .wait()
        .map_err(|e| format!("Failed to wait for process: {}", e))?;

    Ok(())
}
