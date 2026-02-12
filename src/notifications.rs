use futures_util::StreamExt;
use zbus::{Connection, proxy};

/// DBus proxy for freedesktop notifications
#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    fn close_notification(&self, id: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: &str);
}

fn handle_action(action_key: &str, file_path: &str) {
    match action_key {
        "copy-path" => match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(file_path) {
                    tracing::error!("Failed to copy to clipboard: {}", e);
                } else {
                    tracing::info!("Copied path to clipboard: {}", file_path);
                }
            }
            Err(e) => tracing::error!("Failed to create clipboard: {}", e),
        },
        "open-file" => {
            match std::process::Command::new("xdg-open")
                .arg(file_path)
                .spawn()
            {
                Ok(_) => tracing::info!("Opened file: {}", file_path),
                Err(e) => tracing::error!("Failed to open file: {}", e),
            }
        }
        _ => tracing::warn!("Unknown action: {}", action_key),
    }
}

/// Show a notification that recording stopped with action buttons
pub async fn notify_recording_stopped(
    file_path: &str,
    tokio_handle: &tokio::runtime::Handle,
) -> Result<(), String> {
    let connection = Connection::session()
        .await
        .map_err(|e| format!("Failed to connect to DBus: {}", e))?;

    let proxy = NotificationsProxy::new(&connection)
        .await
        .map_err(|e| format!("Failed to create notification proxy: {}", e))?;

    let actions: Vec<&str> = vec![
        "copy-path",
        "Copy Path",
        "open-file",
        "Open File",
    ];

    let notification_id = proxy
        .notify(
            "niri-screen-recorder",
            0,
            "video-x-generic",
            "Recording Saved",
            &format!("Saved to: {}", file_path),
            &actions,
            std::collections::HashMap::new(),
            5000,
        )
        .await
        .map_err(|e| format!("Failed to send notification: {}", e))?;

    tracing::info!("Notification sent with id: {}", notification_id);

    let file_path = file_path.to_owned();
    tokio_handle.spawn(async move {
        if let Err(e) = listen_for_action(notification_id, &file_path).await {
            tracing::error!("Error listening for notification action: {}", e);
        }
    });

    Ok(())
}

async fn listen_for_action(notification_id: u32, file_path: &str) -> Result<(), String> {
    let connection = Connection::session()
        .await
        .map_err(|e| format!("Failed to connect to DBus: {}", e))?;

    let proxy = NotificationsProxy::new(&connection)
        .await
        .map_err(|e| format!("Failed to create notification proxy: {}", e))?;

    let mut stream = proxy
        .receive_action_invoked()
        .await
        .map_err(|e| format!("Failed to listen for ActionInvoked: {}", e))?;

    let timeout_duration = tokio::time::Duration::from_secs(6);
    loop {
        match tokio::time::timeout(timeout_duration, stream.next()).await {
            Ok(Some(signal)) => {
                let args = signal
                    .args()
                    .map_err(|e| format!("Failed to get signal args: {}", e))?;
                if args.id == notification_id {
                    handle_action(args.action_key, file_path);
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => {
                tracing::debug!("Notification action listener timed out");
                break;
            }
        }
    }

    Ok(())
}

/// Show an error notification
pub async fn notify_error(message: &str) -> Result<(), String> {
    let connection = Connection::session()
        .await
        .map_err(|e| format!("Failed to connect to DBus: {}", e))?;

    let proxy = NotificationsProxy::new(&connection)
        .await
        .map_err(|e| format!("Failed to create notification proxy: {}", e))?;

    proxy
        .notify(
            "niri-screen-recorder",
            0,
            "dialog-error",
            "Screen Recorder Error",
            message,
            &[], // no actions
            std::collections::HashMap::new(),
            5000,
        )
        .await
        .map_err(|e| format!("Failed to send notification: {}", e))?;

    Ok(())
}
