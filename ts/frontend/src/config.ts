/** MoQ relay URL. Use `http://` for local dev (self-signed cert fetched automatically). */
export const RELAY_URL: string = "http://localhost:4443";

/** Broadcast path prefix. Layout: `moq-keycast/{roomId}/{username}` → tracks `typing`, `messages`. */
export const MOQ_PATH_PREFIX = "moq-keycast";

/** Track name for live keystroke-by-keystroke typing state. */
export const TRACK_TYPING = "typing";

/** Track name for committed (sent) messages. */
export const TRACK_MESSAGES = "messages";

/** Priority for both tracks (0 = highest). */
export const TRACK_PRIORITY = 0;
