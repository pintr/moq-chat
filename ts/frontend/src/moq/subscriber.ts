import * as Moq from "@moq/lite";
import type { MoqConnection } from "./connection";
import {
    MOQ_PATH_PREFIX,
    TRACK_MESSAGES,
    TRACK_PRIORITY,
    TRACK_TYPING,
} from "../config";
import type {
    ChatMessage,
    MessagePayload,
    OnMessageReceived,
    OnTypingUpdate,
    OnUserJoined,
    OnUserLeft,
    TypingPayload,
} from "../types";
import { connectToRelay } from "./connection";

let abortController: AbortController | null = null;

/**
 * Starts watching the relay for other users in the given room.
 * Returns immediately; all work runs in background async tasks.
 */
export async function startSubscribing(
    roomId: string,
    localUsername: string,
    onTypingUpdate: OnTypingUpdate,
    onMessage: OnMessageReceived,
    onUserJoined: OnUserJoined,
    onUserLeft: OnUserLeft
): Promise<void> {
    const connection = await connectToRelay();

    abortController = new AbortController();
    const { signal } = abortController;

    const roomPrefix = Moq.Path.from(`${MOQ_PATH_PREFIX}/${roomId}/`);
    console.log(`[Subscriber] Watching announcements with prefix: ${roomPrefix}`);

    const announced = connection.announced(roomPrefix);
    void watchAnnouncements(
        connection, announced, localUsername, roomId, signal,
        onTypingUpdate, onMessage, onUserJoined, onUserLeft
    );
}

/** Stops all subscriber background tasks. */
export function stopSubscribing(): void {
    if (!abortController) return;
    console.log("[Subscriber] Stopping all subscriptions…");
    abortController.abort();
    abortController = null;
}

async function watchAnnouncements(
    connection: MoqConnection,
    announced: Moq.Announced,
    localUsername: string,
    roomId: string,
    signal: AbortSignal,
    onTypingUpdate: OnTypingUpdate,
    onMessage: OnMessageReceived,
    onUserJoined: OnUserJoined,
    onUserLeft: OnUserLeft
): Promise<void> {
    const userAbortControllers = new Map<string, AbortController>();

    for (; ;) {
        if (signal.aborted) break;

        const entry = await announced.next();
        if (!entry) {
            console.log("[Subscriber] Announced stream closed.");
            break;
        }

        const pathStr = entry.path.toString();
        const username = pathStr.split("/").at(-1) ?? pathStr;

        if (username === localUsername) continue;

        if (entry.active) {
            console.log(`[Subscriber] User joined: ${username}`);

            userAbortControllers.get(username)?.abort();
            const userAbort = new AbortController();
            userAbortControllers.set(username, userAbort);

            onUserJoined(username);

            const remoteBroadcast = connection.consume(entry.path);
            const typingTrack = remoteBroadcast.subscribe(TRACK_TYPING, TRACK_PRIORITY);
            const messagesTrack = remoteBroadcast.subscribe(TRACK_MESSAGES, TRACK_PRIORITY);

            void readTypingUpdates(typingTrack, username, userAbort.signal, onTypingUpdate);
            void readMessages(messagesTrack, username, roomId, localUsername, userAbort.signal, onMessage);
        } else {
            console.log(`[Subscriber] User left: ${username}`);
            userAbortControllers.get(username)?.abort();
            userAbortControllers.delete(username);
            onUserLeft(username);
        }
    }

    for (const ctrl of userAbortControllers.values()) {
        ctrl.abort();
    }
}

async function readTypingUpdates(
    track: Moq.Track,
    username: string,
    signal: AbortSignal,
    onTypingUpdate: OnTypingUpdate
): Promise<void> {
    console.log(`[Subscriber] Reading typing track for ${username}`);

    for (; ;) {
        if (signal.aborted) break;

        const group = await track.nextGroup();
        if (!group) {
            console.log(`[Subscriber] Typing track closed for ${username}`);
            onTypingUpdate(username, "");
            break;
        }

        const payload = (await group.readJson()) as TypingPayload | undefined;
        if (!payload) continue;

        onTypingUpdate(username, payload.text);
    }
}

async function readMessages(
    track: Moq.Track,
    username: string,
    roomId: string,
    localUsername: string,
    signal: AbortSignal,
    onMessage: OnMessageReceived
): Promise<void> {
    console.log(`[Subscriber] Reading messages track for ${username}`);

    for (; ;) {
        if (signal.aborted) break;

        const group = await track.nextGroup();
        if (!group) {
            console.log(`[Subscriber] Messages track closed for ${username}`);
            break;
        }

        const payload = (await group.readJson()) as MessagePayload | undefined;
        if (!payload) continue;

        const message: ChatMessage = {
            id: `${username}-${payload.timestamp}`,
            username: payload.username,
            text: payload.text,
            timestamp: new Date(payload.timestamp),
            isSelf: false,
        };

        onMessage(message);
    }
}
