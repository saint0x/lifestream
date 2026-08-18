import { useEffect, useMemo, useRef, useState } from "react";
import { Send, Smile, Settings, Users, ChevronRight } from "lucide-react";
import { getAccessToken, getApiWebSocketBaseUrl, requestJson } from "@/lib/api";
import { repository } from "@/lib/repository";
import type { ChatMessage, LiveModerationAction, ViewerPreview } from "@/types";
import "./LiveChat.css";

interface LiveChatProps {
  readonly streamId: string;
  readonly streamTitle: string;
  readonly viewerCount: number;
}

type ChatMode = "chat" | "users";
type ConnectionState = "connecting" | "live" | "reconnecting" | "offline";

type WsEvent =
  | {
      readonly type: "sessionReady";
      readonly sessionToken: string;
      readonly resumed: boolean;
      readonly lastSeenAt: string | null;
    }
  | {
      readonly type: "chatHistory";
      readonly messages: ReadonlyArray<ChatMessage>;
    }
  | {
      readonly type: "chatReplay";
      readonly afterSeq: number;
      readonly messages: ReadonlyArray<ChatMessage>;
    }
  | {
      readonly type: "chatMessage";
      readonly message: ChatMessage;
    }
  | {
      readonly type: "viewerCount";
      readonly viewerCount: number;
    }
  | {
      readonly type: "moderationAction";
      readonly action: LiveModerationAction;
    };

const MAX_MESSAGES = 200;
const VIEWER_PREVIEW_REFRESH_MS = 15_000;
const WS_RECONNECT_DELAY_MS = 1_500;

function dedupeMessages(messages: ReadonlyArray<ChatMessage>): ReadonlyArray<ChatMessage> {
  return Array.from(new Map(messages.map((message) => [message.id, message])).values())
    .sort((left, right) => left.sequence - right.sequence)
    .slice(-MAX_MESSAGES);
}

export function LiveChat({ streamId, streamTitle: _title, viewerCount }: LiveChatProps) {
  const currentUser = useMemo(() => repository.getCurrentUser(), []);
  const [messages, setMessages] = useState<ReadonlyArray<ChatMessage>>([]);
  const [input, setInput] = useState("");
  const [paused, setPaused] = useState(false);
  const [mode, setMode] = useState<ChatMode>("chat");
  const [connectionState, setConnectionState] = useState<ConnectionState>("connecting");
  const [viewerPreview, setViewerPreview] = useState<ViewerPreview>({
    totalViewers: viewerCount,
    sampleUsers: [],
  });
  const [isSending, setIsSending] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);
  const [chatError, setChatError] = useState<string | null>(null);
  const [activeModerationAction, setActiveModerationAction] = useState<LiveModerationAction | null>(
    null,
  );
  const listRef = useRef<HTMLDivElement>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<number | null>(null);
  const disposedRef = useRef(false);
  const sessionTokenRef = useRef<string | null>(null);
  const lastSequenceRef = useRef(0);

  useEffect(() => {
    if (paused) return;
    const element = listRef.current;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [messages, paused]);

  useEffect(() => {
    let cancelled = false;

    const refreshViewerPreview = async () => {
      try {
        const preview = await requestJson<ViewerPreview>(
          `/api/v1/live/streams/${streamId}/viewers`,
          { auth: false },
        );
        if (!cancelled) {
          setViewerPreview(preview);
        }
      } catch (error) {
        if (!cancelled) {
          setViewerPreview((current) => ({
            ...current,
            totalViewers: current.totalViewers || viewerCount,
          }));
        }
      }
    };

    void refreshViewerPreview();
    const intervalId = window.setInterval(() => {
      void refreshViewerPreview();
    }, VIEWER_PREVIEW_REFRESH_MS);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [streamId, viewerCount]);

  useEffect(() => {
    setMessages([]);
    setSendError(null);
    setChatError(null);
    setActiveModerationAction(null);
    sessionTokenRef.current = null;
    lastSequenceRef.current = 0;
    disposedRef.current = false;

    const cleanupSocket = () => {
      if (reconnectTimerRef.current !== null) {
        window.clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      socketRef.current?.close();
      socketRef.current = null;
    };

    const scheduleReconnect = () => {
      if (disposedRef.current || reconnectTimerRef.current !== null) return;
      setConnectionState("reconnecting");
      reconnectTimerRef.current = window.setTimeout(() => {
        reconnectTimerRef.current = null;
        connect();
      }, WS_RECONNECT_DELAY_MS);
    };

    const connect = () => {
      if (disposedRef.current) return;
      setConnectionState((current) => (current === "live" ? "reconnecting" : "connecting"));

      const params = new URLSearchParams();
      const accessToken = getAccessToken();
      if (accessToken) params.set("access_token", accessToken);
      if (sessionTokenRef.current) params.set("session_token", sessionTokenRef.current);
      if (lastSequenceRef.current > 0) params.set("after_seq", String(lastSequenceRef.current));

      const suffix = params.size > 0 ? `?${params.toString()}` : "";
      const socket = new WebSocket(
        `${getApiWebSocketBaseUrl()}/ws/live/${encodeURIComponent(streamId)}${suffix}`,
      );
      socketRef.current = socket;

      socket.addEventListener("open", () => {
        if (!disposedRef.current) {
          setChatError(null);
        }
      });

      socket.addEventListener("message", (event) => {
        const payload = JSON.parse(event.data as string) as WsEvent;
        switch (payload.type) {
          case "sessionReady":
            sessionTokenRef.current = payload.sessionToken;
            setConnectionState("live");
            return;
          case "chatHistory":
            setMessages(dedupeMessages(payload.messages));
            lastSequenceRef.current = payload.messages.at(-1)?.sequence ?? lastSequenceRef.current;
            return;
          case "chatReplay":
            setMessages((current) => dedupeMessages([...current, ...payload.messages]));
            lastSequenceRef.current = payload.messages.at(-1)?.sequence ?? lastSequenceRef.current;
            return;
          case "chatMessage":
            lastSequenceRef.current = Math.max(lastSequenceRef.current, payload.message.sequence);
            setMessages((current) => dedupeMessages([...current, payload.message]));
            setViewerPreview((current) => ({
              ...current,
              totalViewers: Math.max(current.totalViewers, 1),
            }));
            return;
          case "viewerCount":
            setViewerPreview((current) => ({
              ...current,
              totalViewers: payload.viewerCount,
            }));
            return;
          case "moderationAction":
            if (payload.action.subjectUserId !== currentUser.id) {
              return;
            }
            if (payload.action.state === "revoked" || payload.action.revokedAt) {
              setActiveModerationAction(null);
              setSendError(null);
              return;
            }
            setActiveModerationAction(payload.action);
            return;
        }
      });

      socket.addEventListener("close", () => {
        if (socketRef.current === socket) {
          socketRef.current = null;
        }
        if (!disposedRef.current) {
          setChatError("Live chat is reconnecting.");
          scheduleReconnect();
        }
      });

      socket.addEventListener("error", () => {
        if (!disposedRef.current) {
          setChatError("Live chat connection failed.");
        }
      });
    };

    connect();

    return () => {
      disposedRef.current = true;
      cleanupSocket();
    };
  }, [currentUser.id, streamId]);

  const send = async () => {
    const body = input.trim();
    if (!body || isSending) return;
    if (
      activeModerationAction &&
      (activeModerationAction.actionType === "mute" || activeModerationAction.actionType === "ban")
    ) {
      setSendError(`You cannot chat right now: ${activeModerationAction.reason}.`);
      return;
    }

    setIsSending(true);
    setSendError(null);
    try {
      const message = await requestJson<ChatMessage>(
        `/api/v1/live/streams/${streamId}/chat/messages`,
        {
          method: "POST",
          body: {
            body,
            color: "#fafafa",
          },
        },
      );
      lastSequenceRef.current = Math.max(lastSequenceRef.current, message.sequence);
      setMessages((current) => dedupeMessages([...current, message]));
      setInput("");
    } catch (error) {
      setSendError(error instanceof Error ? error.message : "Unable to send message.");
    } finally {
      setIsSending(false);
    }
  };

  const userRows = useMemo(() => {
    const ordered = [...viewerPreview.sampleUsers];
    for (const message of [...messages].reverse()) {
      if (ordered.length >= 20) break;
      ordered.push(message.userHandle);
    }

    const seen = new Set<string>();
    const unique = ordered.filter((handle) => {
      if (!handle) return false;
      if (seen.has(handle)) return false;
      seen.add(handle);
      return true;
    });

    if (currentUser.handle && !seen.has(currentUser.handle)) {
      unique.unshift(currentUser.handle);
      seen.add(currentUser.handle);
    }

    return unique.filter((handle) => {
      if (!handle) return false;
      return true;
    }).slice(0, 20);
  }, [currentUser.handle, messages, viewerPreview.sampleUsers]);

  const composeDisabled =
    isSending ||
    (activeModerationAction !== null &&
      (activeModerationAction.actionType === "mute" || activeModerationAction.actionType === "ban"));

  const moderationNotice = activeModerationAction
    ? activeModerationAction.actionType === "shadowban"
      ? `Restricted: ${activeModerationAction.reason}. Messages may only be visible to you.`
      : `Restricted: ${activeModerationAction.reason}.`
    : null;
  const moderationExpiry = activeModerationAction?.expiresAt
    ? new Date(activeModerationAction.expiresAt).toLocaleString("en-US", {
        month: "short",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
      })
    : null;

  return (
    <aside className="ls-chat">
      <header className="ls-chat__head">
        <div className="ls-chat__tabs">
          <button
            type="button"
            className={mode === "chat" ? "is-active" : ""}
            onClick={() => setMode("chat")}
          >
            Stream Chat
          </button>
          <button
            type="button"
            className={mode === "users" ? "is-active" : ""}
            onClick={() => setMode("users")}
          >
            <Users size={13} />
          </button>
        </div>
        <div className="ls-chat__status-row">
          <div className={`ls-chat__status ls-chat__status--${connectionState}`}>
            {connectionState}
          </div>
          <button type="button" className="ls-chat__settings" aria-label="Chat settings">
            <Settings size={14} />
          </button>
        </div>
      </header>

      {mode === "chat" ? (
        <>
          <div className="ls-chat__list scroll-y" ref={listRef}>
            {messages.map((message) => (
              <div key={message.id} className="ls-chat__msg">
                <span className="ls-chat__badges">
                  {message.badges.map((badge) => (
                    <span key={badge} className={`ls-chat__badge ls-chat__badge--${badge}`}>
                      {badge[0]?.toUpperCase()}
                    </span>
                  ))}
                </span>
                <span className="ls-chat__user" style={{ color: message.color }}>
                  {message.displayName}
                </span>
                <span className="ls-chat__sep">:</span>
                <span className="ls-chat__body">{message.body}</span>
              </div>
            ))}
            {messages.length === 0 ? (
              <div className="ls-chat__empty">No messages yet. Be the first one in chat.</div>
            ) : null}
          </div>

          {paused ? (
            <button
              type="button"
              className="ls-chat__resume"
              onClick={() => setPaused(false)}
            >
              Chat paused — click to resume <ChevronRight size={12} />
            </button>
          ) : null}

          {moderationNotice ? (
            <div className="ls-chat__moderation mono">
              {activeModerationAction?.actionType}
              {moderationExpiry ? ` until ${moderationExpiry}` : " active"}
            </div>
          ) : null}
          {chatError ? <div className="ls-chat__notice">{chatError}</div> : null}
          {moderationNotice ? <div className="ls-chat__notice">{moderationNotice}</div> : null}
          {sendError ? <div className="ls-chat__notice">{sendError}</div> : null}

          <form
            className="ls-chat__compose"
            onSubmit={(event) => {
              event.preventDefault();
              void send();
            }}
          >
            <input
              value={input}
              onChange={(event) => setInput(event.target.value)}
              onFocus={() => setPaused(true)}
              onBlur={() => setPaused(false)}
              placeholder={composeDisabled ? "Chat is restricted right now" : "Send a message…"}
              aria-label="Chat message"
              disabled={composeDisabled}
            />
            <button type="button" className="ls-chat__icon-btn" aria-label="Emotes">
              <Smile size={14} />
            </button>
            <button
              type="submit"
              className="ls-chat__send"
              disabled={!input.trim() || composeDisabled}
              aria-label="Send"
            >
              <Send size={13} />
            </button>
          </form>
        </>
      ) : (
        <div className="ls-chat__users scroll-y">
          <div className="ls-chat__users-head mono">
            {viewerPreview.totalViewers.toLocaleString()} viewers
          </div>
          {userRows.map((handle) => (
            <div key={handle} className="ls-chat__user-row">
              <span className="ls-chat__user-dot" />
              {handle}
            </div>
          ))}
        </div>
      )}
    </aside>
  );
}
