use anyhow::Context;
use clap::Parser;
use tokio::sync::mpsc;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use url::Url;

mod publish;
mod subscribe;
mod tui;

pub use tui::PeerEvent;

#[derive(Parser)]
#[command(name = "moq-keycast", about = "Live keystroke broadcast over MoQ/QUIC")]
struct Args {
    /// Relay server URL
    #[arg(long, default_value = "https://localhost:4443")]
    relay: Url,

    /// Chat room name
    #[arg(long, default_value = "general")]
    room: String,

    /// Your display name
    #[arg(long)]
    username: String,

    #[command(flatten)]
    client: moq_native::ClientConfig,

    #[command(flatten)]
    log: moq_native::Log,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Log to a file — writing to stderr corrupts the ratatui alternate-screen buffer.
    let log_file =
        std::fs::File::create("moq-keycast.log").context("failed to create moq-keycast.log")?;
    let filter = EnvFilter::builder()
        .with_default_directive(args.log.level().into())
        .from_env_lossy();
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(log_file).with_filter(filter))
        .init();

    let client = args
        .client
        .init()
        .context("failed to initialise MoQ client")?;

    let (typing_tx, typing_rx) = mpsc::unbounded_channel::<String>();
    let (peer_tx, peer_rx) = mpsc::unbounded_channel::<PeerEvent>();

    let broadcast_name = format!("moq-keycast/{}/{}", args.room, args.username);

    let pub_client = client.clone();
    let pub_relay = args.relay.clone();
    let pub_handle = tokio::spawn(async move {
        if let Err(e) = publish::run(pub_client, pub_relay, broadcast_name, typing_rx).await {
            tracing::error!("publisher: {e:#}");
        }
    });

    let sub_client = client;
    let sub_relay = args.relay.clone();
    let room = args.room.clone();
    let own_username = args.username.clone();
    let sub_handle = tokio::spawn(async move {
        if let Err(e) = subscribe::run(sub_client, sub_relay, room, own_username, peer_tx).await {
            tracing::error!("subscriber: {e:#}");
        }
    });

    tui::run(args.room, args.username, typing_tx, peer_rx).await?;

    pub_handle.abort();
    sub_handle.abort();

    Ok(())
}
