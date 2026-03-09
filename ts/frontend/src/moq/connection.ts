import * as Moq from "@moq/lite";
import { RELAY_URL } from "../config";

/**
 * Convenience alias for the return type of `Moq.Connection.connect()`.
 */
export type MoqConnection = Awaited<ReturnType<typeof Moq.Connection.connect>>;

/** Singleton MoQ connection shared by all publishers and subscribers. */
let connection: MoqConnection | null = null;

/**
 * Establishes a connection to the moq-relay server.
 * Returns an existing connection if one is already open.
 * @throws If the relay is unreachable or the handshake fails.
 */
export async function connectToRelay(): Promise<MoqConnection> {
    if (connection) return connection;

    console.log(`[MoQ] Connecting to relay at ${RELAY_URL}…`);
    const url = new URL(RELAY_URL);

    connection = await Moq.Connection.connect(url);

    console.log(`[MoQ] Connected!`);

    connection.closed.then(() => {
        console.warn("[MoQ] Connection closed by relay.");
        connection = null;
    });

    return connection;
}

/** Returns the current connection or throws if not yet connected. */
export function getConnection(): MoqConnection {
    if (!connection) {
        throw new Error("[MoQ] Not connected. Call connectToRelay() first.");
    }
    return connection;
}

/** Closes the relay connection. Call on page unload or room exit. */
export function disconnectFromRelay(): void {
    if (!connection) return;
    console.log("[MoQ] Disconnecting from relay…");
    connection.close();
    connection = null;
}
