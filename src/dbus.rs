use std::process::Child;
use std::sync::Arc;
use tokio::sync::RwLock;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, interface};

use crate::notifications;
use crate::recorder;

/// State shared between DBus methods
struct RecorderState {
    recording: bool,
    current_file: Option<String>,
    child: Option<Child>,
}

impl Default for RecorderState {
    fn default() -> Self {
        Self {
            recording: false,
            current_file: None,
            child: None,
        }
    }
}

/// The DBus interface exposed to clients
struct ScreenRecorder {
    state: Arc<RwLock<RecorderState>>,
    tokio_handle: tokio::runtime::Handle,
}

#[interface(name = "org.matthew_hre.NiriScreenRecorder")]
impl ScreenRecorder {
    /// Start a new recording
    async fn start_recording(&self, #[zbus(signal_context)] ctxt: SignalEmitter<'_>) -> bool {
        let mut state = self.state.write().await;

        if state.recording {
            tracing::warn!("Already recording, ignoring start request");
            return false;
        }

        // Select region
        let region = match recorder::select_region() {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to select region: {}", e);
                notifications::notify_error(&e).await.ok();
                return false;
            }
        };

        // Start recording
        match recorder::start_recording(&region) {
            Ok((child, file)) => {
                state.recording = true;
                state.current_file = Some(file.clone());
                state.child = Some(child);

                tracing::info!("Recording started: {}", file);

                // Emit signal
                Self::recording_started(&ctxt).await.ok();
                true
            }
            Err(e) => {
                tracing::error!("Failed to start recording: {}", e);
                notifications::notify_error(&e).await.ok();
                false
            }
        }
    }

    /// Stop the current recording
    async fn stop_recording(&self, #[zbus(signal_context)] ctxt: SignalEmitter<'_>) -> bool {
        let mut state = self.state.write().await;

        if !state.recording {
            tracing::warn!("Not recording, ignoring stop request");
            return false;
        }

        let file = state.current_file.clone().unwrap_or_default();

        // Stop the recording process
        if let Some(ref mut child) = state.child {
            if let Err(e) = recorder::stop_recording(child) {
                tracing::error!("Failed to stop recording: {}", e);
            }
        }

        state.recording = false;
        state.current_file = None;
        state.child = None;

        tracing::info!("Recording stopped: {}", file);

        // Emit signal with the file path
        Self::recording_stopped(&ctxt, &file).await.ok();

        // Send notification
        notifications::notify_recording_stopped(&file, &self.tokio_handle)
            .await
            .ok();

        true
    }

    /// Toggle recording on/off
    async fn toggle_recording(&self, #[zbus(signal_context)] ctxt: SignalEmitter<'_>) -> bool {
        let state = self.state.read().await;

        if state.recording {
            drop(state);
            self.stop_recording(ctxt).await
        } else {
            drop(state);
            self.start_recording(ctxt).await
        }
    }

    /// Check if currently recording
    async fn is_recording(&self) -> bool {
        self.state.read().await.recording
    }

    /// Get the current recording file path
    async fn get_current_file(&self) -> String {
        self.state
            .read()
            .await
            .current_file
            .clone()
            .unwrap_or_default()
    }

    /// Signal emitted when recording starts
    #[zbus(signal)]
    async fn recording_started(ctxt: &SignalEmitter<'_>) -> zbus::Result<()>;

    /// Signal emitted when recording stops, includes file path
    #[zbus(signal)]
    async fn recording_stopped(ctxt: &SignalEmitter<'_>, file_path: &str) -> zbus::Result<()>;
}

/// Run the daemon (server mode)
pub async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Starting niri-screen-recorder daemon");

    let state = Arc::new(RwLock::new(RecorderState::default()));
    let tokio_handle = tokio::runtime::Handle::current();
    let recorder = ScreenRecorder {
        state,
        tokio_handle,
    };

    // Connect to the session bus
    let connection = Connection::session().await?;

    // Register our service name
    connection
        .object_server()
        .at("/org/matthew_hre/NiriScreenRecorder", recorder)
        .await?;

    connection
        .request_name("org.matthew_hre.NiriScreenRecorder")
        .await?;

    tracing::info!("DBus service registered, waiting for requests...");

    // Run forever
    std::future::pending::<()>().await;

    Ok(())
}

/// Client: call StartRecording on the daemon
pub async fn call_start() -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::session().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.matthew_hre.NiriScreenRecorder",
        "/org/matthew_hre/NiriScreenRecorder",
        "org.matthew_hre.NiriScreenRecorder",
    )
    .await?;

    let result: Result<bool, _> = proxy.call("StartRecording", &()).await;
    match result {
        Ok(started) => {
            if started {
                println!("Recording started");
            } else {
                eprintln!(
                    "Failed to start recording (already recording or region selection failed)"
                );
            }
        }
        Err(e) => {
            eprintln!(
                "Error: Could not connect to daemon. Is it running? (niri-screen-recorder daemon)"
            );
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Client: call StopRecording on the daemon
pub async fn call_stop() -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::session().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.matthew_hre.NiriScreenRecorder",
        "/org/matthew_hre/NiriScreenRecorder",
        "org.matthew_hre.NiriScreenRecorder",
    )
    .await?;

    let result: Result<bool, _> = proxy.call("StopRecording", &()).await;
    match result {
        Ok(stopped) => {
            if stopped {
                println!("Recording stopped");
            } else {
                eprintln!("No recording in progress");
            }
        }
        Err(e) => {
            eprintln!(
                "Error: Could not connect to daemon. Is it running? (niri-screen-recorder daemon)"
            );
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Client: call ToggleRecording on the daemon
pub async fn call_toggle() -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::session().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.matthew_hre.NiriScreenRecorder",
        "/org/matthew_hre/NiriScreenRecorder",
        "org.matthew_hre.NiriScreenRecorder",
    )
    .await?;

    let result: Result<bool, _> = proxy.call("ToggleRecording", &()).await;
    match result {
        Ok(_) => {}
        Err(e) => {
            eprintln!(
                "Error: Could not connect to daemon. Is it running? (niri-screen-recorder daemon)"
            );
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Client: check recording status
pub async fn call_status() -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::session().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.matthew_hre.NiriScreenRecorder",
        "/org/matthew_hre/NiriScreenRecorder",
        "org.matthew_hre.NiriScreenRecorder",
    )
    .await?;

    let recording: Result<bool, _> = proxy.call("IsRecording", &()).await;
    let recording = match recording {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Error: Could not connect to daemon. Is it running? (niri-screen-recorder daemon)"
            );
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    };

    let file: Result<String, _> = proxy.call("GetCurrentFile", &()).await;
    let file = match file {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Error: Could not connect to daemon. Is it running? (niri-screen-recorder daemon)"
            );
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    };

    if recording {
        println!("Recording: yes");
        println!("File: {}", file);
    } else {
        println!("Recording: no");
    }

    Ok(())
}
