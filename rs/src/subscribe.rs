use anyhow::Context;
use std::collections::HashMap;
use tokio::sync::mpsc;
use url::Url;

use crate::PeerEvent;

/// Watch for other users in the same room and forward their typing to `peer_tx`.
pub async fn run(
    client: moq_native::Client,
    relay: Url,
    room: String,
    own_username: String,
    peer_tx: mpsc::UnboundedSender<PeerEvent>,
) -> anyhow::Result<()> {
    let origin = moq_lite::Origin::produce();

    let session = client
        .with_consume(origin.clone())
        .connect(relay)
        .await
        .context("subscriber connect")?;

    let room_prefix = format!("moq-chat/{room}");
    let username_prefix = format!("{room_prefix}/");
    let room_path: moq_lite::PathOwned = room_prefix.into();

    let mut consumer = origin
        .consume_only(&[room_path])
        .context("failed to create origin consumer")?;

    tracing::info!(%room, "subscriber watching room");

    let mut peer_tasks: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();
    let track_def = moq_lite::Track {
        name: "typing".to_string(),
        priority: 0,
    };

    loop {
        tokio::select! {
            res = session.closed() => return res.context("subscriber session closed"),
            Some((path, maybe_broadcast)) = consumer.announced() => {
                let path_str = path.as_str().to_string();
                let Some(username) = path_str.strip_prefix(&username_prefix) else { continue };
                let username = username.to_string();

                if username == own_username { continue; }

                match maybe_broadcast {
                    Some(broadcast) => {
                        if let Some(prev) = peer_tasks.remove(&username) { prev.abort(); }

                        let Ok(track) = broadcast.subscribe_track(&track_def) else {
                            tracing::warn!(%username, "subscribe_track failed");
                            continue;
                        };

                        let _ = peer_tx.send(PeerEvent::Joined(username.clone()));

                        let tx = peer_tx.clone();
                        let uname = username.clone();
                        let handle = tokio::spawn(async move {
                            if let Err(e) = read_peer_track(uname.clone(), track, tx).await {
                                tracing::debug!(%uname, "track reader ended: {e}");
                            }
                        });
                        peer_tasks.insert(username, handle);
                    }
                    None => {
                        if let Some(prev) = peer_tasks.remove(&username) { prev.abort(); }
                        let _ = peer_tx.send(PeerEvent::Offline(username));
                    }
                }
            }
        }
    }
}

async fn read_peer_track(
    username: String,
    mut track: moq_lite::TrackConsumer,
    peer_tx: mpsc::UnboundedSender<PeerEvent>,
) -> anyhow::Result<()> {
    while let Some(mut group) = track.next_group().await? {
        while let Some(frame) = group.read_frame().await? {
            let _ = peer_tx.send(PeerEvent::Update(
                username.clone(),
                parse_typing_frame(&frame),
            ));
        }
    }
    Ok(())
}

/// Decode a typing frame: tries JSON `{"text":"..."}` first, falls back to raw UTF-8.
fn parse_typing_frame(frame: &[u8]) -> String {
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(frame) {
        if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
            return text.to_string();
        }
    }
    String::from_utf8_lossy(frame).into_owned()
}
