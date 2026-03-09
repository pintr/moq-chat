import "./ui/styles.css";

import { connectToRelay, disconnectFromRelay } from "./moq/connection";
import {
    publishTypingUpdate,
    startPublishing,
    stopPublishing,
} from "./moq/publisher";
import {
    startSubscribing,
    stopSubscribing,
} from "./moq/subscriber";
import { renderChatRoom } from "./ui/chat-room";
import { renderLobby } from "./ui/lobby";

const app = document.getElementById("app")!;

renderLobby(app, onJoin);

async function onJoin(username: string, roomId: string): Promise<void> {
    await connectToRelay();
    await startPublishing(roomId, username);

    const roomUI = renderChatRoom(app, roomId, username, onLeave);

    app.addEventListener("moq:typing", (e) => {
        const text = (e as CustomEvent<string>).detail;
        publishTypingUpdate(text);
    });

    await startSubscribing(
        roomId,
        username,
        (remoteUsername, text) => { roomUI.updateTyping(remoteUsername, text); },
        () => { },
        (remoteUsername) => { roomUI.addRemoteUser(remoteUsername); },
        (remoteUsername) => { roomUI.removeRemoteUser(remoteUsername); },
    );
}

function onLeave(): void {
    console.log("[App] Leaving room…");
    stopSubscribing();
    stopPublishing();
    disconnectFromRelay();
    renderLobby(app, onJoin);
}

window.addEventListener("beforeunload", () => {
    stopSubscribing();
    stopPublishing();
    disconnectFromRelay();
});
