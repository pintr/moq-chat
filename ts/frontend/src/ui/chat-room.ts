// =============================================================================
// Chat Room UI Component
// =============================================================================
//
// Layout:
//   1. Header  – room name and a "Leave" button.
//   2. Body    – one live-feed card per remote user.  Each card shows whatever
//                that user is currently typing, updated keystroke-by-keystroke.
//   3. Footer  – the local user's text input.
//
// There is NO send button and NO Enter-to-send.  Every `input` event on the
// local text field dispatches a `moq:typing` CustomEvent which main.ts
// forwards to `publishTypingUpdate(text)`.  Remote users receive each group
// on the "typing" MoQ track and their feed card is updated in real-time.
//
// =============================================================================

import type { OnTypingUpdate, OnUserJoined, OnUserLeft } from "../types";

/** Callback fired when the user clicks "Leave" (or closes the page). */
export type OnLeave = () => void;

/**
 * Renders the live-typing chat room and wires up all callbacks.
 *
 * @param container - DOM element to render into (replaces lobby content).
 * @param roomId    - Room ID shown in the header.
 * @param username  - Local user's display name (shown in the footer label).
 * @param onLeave   - Called when the user clicks the Leave button.
 *
 * @returns Methods the caller invokes as MoQ events arrive:
 *   - `addRemoteUser(username)` – create a live-feed card for a new peer
 *   - `removeRemoteUser(username)` – remove a peer's feed card
 *   - `updateTyping(username, text)` – update a peer's feed card text live
 */
export function renderChatRoom(
  container: HTMLElement,
  roomId: string,
  username: string,
  onLeave: OnLeave
): {
  addRemoteUser: OnUserJoined;
  removeRemoteUser: OnUserLeft;
  updateTyping: OnTypingUpdate;
} {
  // ── Build HTML structure ─────────────────────────────────────────────────────
  container.innerHTML = `
    <div class="chat-room">
      <!-- ── Header ─────────────────────────────────────────────────────── -->
      <header class="chat-header">
        <div class="chat-header-left">
          <span class="room-badge">Room</span>
          <span class="room-title">${escapeHtml(roomId)}</span>
        </div>
        <button id="leave-btn" class="btn-leave">Leave</button>
      </header>

      <!-- ── Body: one live-feed card per remote peer ───────────────────── -->
      <div class="chat-body">
        <div class="live-feeds" id="live-feeds">
          <p class="no-peers-hint" id="no-peers-hint">Waiting for others to join…</p>
        </div>
      </div>

      <!-- ── Footer: local input — every keystroke is sent via MoQ ──────── -->
      <footer class="chat-footer">
        <span class="local-label">${escapeHtml(username)}</span>
        <textarea
          id="message-input"
          rows="1"
          placeholder="Type something…"
          autocomplete="off"
          spellcheck="true"
          aria-label="Your message"
        ></textarea>
      </footer>
    </div>
  `;

  // ── DOM references ───────────────────────────────────────────────────────────
  const liveFeeds = container.querySelector<HTMLDivElement>("#live-feeds")!;
  const noPeersHint = container.querySelector<HTMLParagraphElement>("#no-peers-hint")!;
  const messageInput = container.querySelector<HTMLTextAreaElement>("#message-input")!;
  const leaveBtn = container.querySelector<HTMLButtonElement>("#leave-btn")!;

  // Auto-grow the textarea as text wraps onto new lines.
  function autoResize() {
    messageInput.style.height = "auto";
    messageInput.style.height = messageInput.scrollHeight + "px";
  }

  // Publish every keystroke immediately — no buffering, no send button.
  messageInput.addEventListener("input", () => {
    autoResize();
    container.dispatchEvent(
      new CustomEvent("moq:typing", { detail: messageInput.value, bubbles: true })
    );
  });

  // Enter clears input (matching Rust TUI behaviour) — no newline inserted.
  messageInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      messageInput.value = "";
      autoResize();
      container.dispatchEvent(
        new CustomEvent("moq:typing", { detail: "", bubbles: true })
      );
    }
  });

  leaveBtn.addEventListener("click", () => { onLeave(); });
  messageInput.focus();

  // ── Return the event handlers for the subscriber ─────────────────────────────

  return {
    addRemoteUser(remoteUsername: string): void {
      if (liveFeeds.querySelector(`[data-username="${CSS.escape(remoteUsername)}"]`)) return;
      noPeersHint.style.display = "none";
      const feed = document.createElement("div");
      feed.className = "user-feed";
      feed.setAttribute("data-username", remoteUsername);
      feed.innerHTML = `
              <div class="feed-label">${escapeHtml(remoteUsername)}</div>
              <div class="feed-text"></div>
            `;
      liveFeeds.appendChild(feed);
    },

    removeRemoteUser(remoteUsername: string): void {
      liveFeeds.querySelector(`[data-username="${CSS.escape(remoteUsername)}"]`)?.remove();
      if (!liveFeeds.querySelector(".user-feed")) {
        noPeersHint.style.display = "";
      }
    },

    updateTyping(remoteUsername: string, text: string): void {
      const feed = liveFeeds.querySelector(`[data-username="${CSS.escape(remoteUsername)}"]`);
      const textEl = feed?.querySelector<HTMLDivElement>(".feed-text");
      if (textEl) textEl.textContent = text;
    },
  };
}

// ── DOM helpers ───────────────────────────────────────────────────────────────

/**
 * Basic XSS guard: escapes HTML special characters in user-supplied strings
 * before inserting them into innerHTML contexts.
 */
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#x27;");
}
