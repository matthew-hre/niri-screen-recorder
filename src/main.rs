mod dbus;
mod notifications;
mod recorder;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "niri-screen-recorder")]
#[command(about = "Screen recorder daemon for niri", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon in the background
    Daemon,
    /// Start a recording
    Start,
    /// Stop the current recording
    Stop,
    /// Toggle recording on/off
    Toggle,
    /// Show recording status
    Status,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon => {
            dbus::run_daemon().await?;
        }
        Commands::Start => {
            dbus::call_start().await?;
        }
        Commands::Stop => {
            dbus::call_stop().await?;
        }
        Commands::Toggle => {
            dbus::call_toggle().await?;
        }
        Commands::Status => {
            dbus::call_status().await?;
        }
    }

    Ok(())
}
