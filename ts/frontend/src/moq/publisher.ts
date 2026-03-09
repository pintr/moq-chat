import * as Moq from "@moq/lite";
import {
    MOQ_PATH_PREFIX,
    TRACK_MESSAGES,
    TRACK_TYPING,
} from "../config";
import type { MessagePayload, TypingPayload } from "../types";
import { connectToRelay } from "./connection";

let broadcast: Moq.Broadcast | null = null;
const activeTypingTracks = new Set<Moq.Track>();
const activeMessageTracks = new Set<Moq.Track>();

/**
 * Starts publishing this user's broadcast at `moq-chat/{roomId}/{username}`.
 * Track subscription requests are served automatically in the background.
 */
export async function startPublishing(
    roomId: string,
    username: string
): Promise<void> {
    const connection = await connectToRelay();
    const broadcastPath = Moq.Path.from(`${MOQ_PATH_PREFIX}/${roomId}/${username}`);
    broadcast = new Moq.Broadcast();
    connection.publish(broadcastPath, broadcast);
    console.log(`[Publisher] Announced broadcast at path: ${broadcastPath}`);
    void serveTrackRequests(broadcast);
}

/** Stops publishing and tears down the broadcast. */
export function stopPublishing(): void {
    if (!broadcast) return;
    console.log("[Publisher] Stopping broadcast…");
    broadcast.close();
    broadcast = null;
    activeTypingTracks.clear();
    activeMessageTracks.clear();
}

/** Publishes the user's current typing text to all active subscribers. */
export function publishTypingUpdate(text: string): void {
    if (activeTypingTracks.size === 0) return;
    const payload: TypingPayload = { text, timestamp: Date.now() };
    for (const track of activeTypingTracks) {
        try {
            const group = track.appendGroup();
            group.writeJson(payload);
            group.close();
        } catch (err) {
            console.warn("[Publisher] Typing track closed, removing:", err);
            activeTypingTracks.delete(track);
        }
    }
}

/** Publishes a committed message to all active subscribers. */
export function publishMessage(text: string, username: string): void {
    const payload: MessagePayload = { text, username, timestamp: Date.now() };
    for (const track of activeMessageTracks) {
        try {
            const group = track.appendGroup();
            group.writeJson(payload);
            group.close();
        } catch (err) {
            console.warn("[Publisher] Message track closed, removing:", err);
            activeMessageTracks.delete(track);
        }
    }
}

/** Accepts incoming track subscription requests for the lifetime of the broadcast. */
async function serveTrackRequests(broadcast: Moq.Broadcast): Promise<void> {
    for (; ;) {
        const request = await broadcast.requested();
        if (!request) {
            console.log("[Publisher] Broadcast closed, stopping request loop.");
            return;
        }

        const { track } = request;
        console.log(`[Publisher] Track requested: "${track.name}"`);

        if (track.name === TRACK_TYPING) {
            activeTypingTracks.add(track);
            // Send an initial empty frame so the subscriber's nextGroup() doesn't block.
            const group = track.appendGroup();
            group.writeJson({ text: "", timestamp: Date.now() } as TypingPayload);
            group.close();
            track.closed.then(() => {
                activeTypingTracks.delete(track);
                console.log("[Publisher] Typing subscriber disconnected.");
            });
        } else if (track.name === TRACK_MESSAGES) {
            activeMessageTracks.add(track);
            track.closed.then(() => {
                activeMessageTracks.delete(track);
                console.log("[Publisher] Message subscriber disconnected.");
            });
        } else {
            console.warn(`[Publisher] Unknown track requested: "${track.name}"`);
            track.close(new Error(`Unknown track: ${track.name}`));
        }
    }
}
