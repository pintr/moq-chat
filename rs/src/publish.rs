use anyhow::Context;
use tokio::sync::mpsc;
use url::Url;

/// Publish our typing to the relay.
///
/// Each keystroke is written as a JSON frame `{"text":"...","timestamp":<ms>}`
/// on the `"typing"` track — matching the TypeScript `TypingPayload` wire format.
/// A no-op `"messages"` track is also created so the TypeScript SPA receives a
/// clean SUBSCRIBE_OK instead of an error when it subscribes to that track.
pub async fn run(
    client: moq_native::Client,
    relay: Url,
    broadcast_name: String,
    mut typing_rx: mpsc::UnboundedReceiver<String>,
) -> anyhow::Result<()> {
    let origin = moq_lite::Origin::produce();

    let mut broadcast = moq_lite::Broadcast::produce();
    let mut track = broadcast
        .create_track(moq_lite::Track { name: "typing".to_string(), priority: 0 })
        .context("create typing track")?;

    // Keep alive for TypeScript SPA compatibility — it subscribes to "messages" on every user.
    let _messages_track = broadcast
        .create_track(moq_lite::Track { name: "messages".to_string(), priority: 0 })
        .context("create messages track")?;

    origin.publish_broadcast(&broadcast_name, broadcast.consume());

    let session = client
        .with_publish(origin.consume())
        .connect(relay)
        .await
        .context("publisher connect")?;

    tracing::info!(broadcast = %broadcast_name, "publisher connected");

    loop {
        tokio::select! {
            res = session.closed() => return res.context("publisher session closed"),
            msg = typing_rx.recv() => {
                let Some(text) = msg else { break };
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let payload = serde_json::json!({"text": text, "timestamp": timestamp});
                if let Err(e) = track.write_frame(payload.to_string()) {
                    tracing::warn!("write_frame: {e}");
                }
            }
        }
    }

    Ok(())
}
