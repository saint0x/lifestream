import { useEffect, useRef, useState } from "react";
import {
  Activity,
  Check,
  Copy,
  Cpu,
  Eye,
  EyeOff,
  HardDrive,
  Link2,
  MessageSquare,
  Radio,
  RefreshCw,
  Square,
  Users,
  Wifi,
} from "lucide-react";
import { CreatorLayout } from "@/components/creator/CreatorLayout";
import { Sparkline } from "@/components/creator/Sparkline";
import { StatCard } from "@/components/creator/StatCard";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { getAccessToken, getApiWebSocketBaseUrl, requestJson } from "@/lib/api";
import { formatUptime, formatViewers } from "@/lib/format";
import type {
  Broadcast,
  CollaborationParticipant,
  CreatorModerator,
  CreatorCollaborationControlResponse,
  CreatorLiveControlResponse,
  CreatorLiveRuntimeResponse,
  CreatorLiveSettings,
  CreatorProfile,
  Genre,
  LiveModerationAction,
  LiveStreamReportRecord,
  ModerationAuditEntry,
} from "@/types";
import "./Creator.css";
import "./CreatorLivePage.css";

const categoryOptions: ReadonlyArray<Genre> = [
  "Tech",
  "Gaming",
  "Music",
  "Talk",
  "Sports",
  "News",
  "Drama",
  "Sci-Fi",
];

const collaborationRoleOptions = [
  { value: "guest", label: "Guest" },
  { value: "co_host", label: "Co-host" },
  { value: "co_streamer", label: "Co-stream" },
] as const;

const collaborationChatModeOptions = [
  { value: "shared", label: "Shared chat" },
  { value: "host_only", label: "Host chat" },
] as const;

const collaborationRecordingPolicyOptions = [
  { value: "host_archive", label: "Host archive" },
  { value: "split_archive", label: "Split archive" },
] as const;

const moderatorRoleOptions = [
  { value: "moderator", label: "Moderator" },
  { value: "senior_moderator", label: "Senior moderator" },
] as const;

const moderationActionTypeOptions = [
  { value: "mute", label: "Mute" },
  { value: "ban", label: "Ban" },
  { value: "shadowban", label: "Shadowban" },
] as const;

const reportStatusOptions = [
  { value: "open", label: "Open" },
  { value: "reviewing", label: "Reviewing" },
  { value: "resolved", label: "Resolved" },
  { value: "dismissed", label: "Dismissed" },
] as const;

function formatTimestamp(value?: string | null): string {
  if (!value) return "n/a";
  return new Date(value).toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function participantStateActionLabel(state: string): string {
  switch (state) {
    case "accepted":
      return "Move backstage";
    case "backstage":
      return "Bring live";
    case "live":
      return "Return backstage";
    case "left":
    case "removed":
      return "Restore backstage";
    default:
      return "Update";
  }
}

function participantStateActionTarget(state: string): string | null {
  switch (state) {
    case "accepted":
      return "backstage";
    case "backstage":
      return "live";
    case "live":
      return "backstage";
    case "left":
    case "removed":
      return "backstage";
    default:
      return null;
  }
}

export function CreatorLivePage() {
  const [control, setControl] = useState<CreatorLiveControlResponse | null>(null);
  const [runtime, setRuntime] = useState<CreatorLiveRuntimeResponse | null>(null);
  const [title, setTitle] = useState("");
  const [category, setCategory] = useState<Genre>("Tech");
  const [tags, setTags] = useState<ReadonlyArray<string>>([]);
  const [tagDraft, setTagDraft] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [copied, setCopied] = useState<"key" | "url" | null>(null);
  const [isMature, setIsMature] = useState(false);
  const [notify, setNotify] = useState(true);
  const [subscriberOnly, setSubscriberOnly] = useState(false);
  const [slowModeSeconds, setSlowModeSeconds] = useState(0);
  const [activeScene, setActiveScene] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [actionPending, setActionPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [socketStatus, setSocketStatus] = useState<"connecting" | "open" | "closed">("connecting");
  const [collabTitle, setCollabTitle] = useState("");
  const [collabChatMode, setCollabChatMode] = useState("shared");
  const [collabRecordingPolicy, setCollabRecordingPolicy] = useState("host_archive");
  const [inviteeUserId, setInviteeUserId] = useState("");
  const [inviteRole, setInviteRole] = useState("guest");
  const [inviteMirror, setInviteMirror] = useState(false);
  const [inviteMessage, setInviteMessage] = useState("");
  const [inviteExpiresMinutes, setInviteExpiresMinutes] = useState(30);
  const [moderators, setModerators] = useState<ReadonlyArray<CreatorModerator>>([]);
  const [moderationActions, setModerationActions] = useState<ReadonlyArray<LiveModerationAction>>([]);
  const [moderationReports, setModerationReports] = useState<ReadonlyArray<LiveStreamReportRecord>>([]);
  const [moderationAudit, setModerationAudit] = useState<ReadonlyArray<ModerationAuditEntry>>([]);
  const [moderatorUserId, setModeratorUserId] = useState("");
  const [moderatorRole, setModeratorRole] = useState("moderator");
  const [actionSubjectUserId, setActionSubjectUserId] = useState("");
  const [actionType, setActionType] = useState("mute");
  const [actionReason, setActionReason] = useState("");
  const [actionDurationMinutes, setActionDurationMinutes] = useState(15);
  const [reportResolutionStatus, setReportResolutionStatus] = useState("reviewing");
  const [reportResolutionNote, setReportResolutionNote] = useState("");
  const dirtyRef = useRef(false);
  const socketSessionTokenRef = useRef<string | null>(null);

  const hydrateDraftFromControl = (nextControl: CreatorLiveControlResponse) => {
    const snapshotBroadcast =
      nextControl.snapshot.currentBroadcast ?? nextControl.snapshot.pendingBroadcast;
    const nextProfile = nextControl.snapshot.profile;
    setTitle(snapshotBroadcast?.title ?? "");
    setCategory((snapshotBroadcast?.category ?? nextProfile.defaultCategory) as Genre);
    setTags(snapshotBroadcast?.tags ?? nextProfile.defaultTags);
    setIsMature(snapshotBroadcast?.isMature ?? false);
    setNotify(nextControl.settings.notifyFollowersDefault);
    setSubscriberOnly(nextControl.settings.subscriberOnly);
    setSlowModeSeconds(nextControl.settings.slowModeSeconds);
    setActiveScene(nextControl.settings.activeSceneId);
    setCollabTitle(snapshotBroadcast?.title ? `${snapshotBroadcast.title} Collab` : "Collaboration Session");
  };

  const applyLiveState = (
    nextControl: CreatorLiveControlResponse,
    nextRuntime: CreatorLiveRuntimeResponse,
    forceDraftSync = false,
  ) => {
    setControl(nextControl);
    setRuntime(nextRuntime);
    if (forceDraftSync || !dirtyRef.current) {
      hydrateDraftFromControl(nextControl);
    }
  };

  const refresh = async () => {
    const [nextControl, nextRuntime] = await Promise.all([
      requestJson<CreatorLiveControlResponse>("/api/v1/creator/me/live/control"),
      requestJson<CreatorLiveRuntimeResponse>("/api/v1/creator/me/live/runtime"),
    ]);
    applyLiveState(nextControl, nextRuntime, true);
  };

  useEffect(() => {
    void (async () => {
      try {
        setLoading(true);
        setError(null);
        await refresh();
      } catch (nextError) {
        setError(nextError instanceof Error ? nextError.message : "Unable to load live control.");
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  useEffect(() => {
    const accessToken = getAccessToken();
    if (!accessToken) {
      setSocketStatus("closed");
      return;
    }

    let cancelled = false;
    let socket: WebSocket | null = null;
    let reconnectTimer: number | null = null;

    const connect = () => {
      if (cancelled) return;
      setSocketStatus("connecting");
      const params = new URLSearchParams();
      params.set("access_token", accessToken);
      if (socketSessionTokenRef.current) {
        params.set("session_token", socketSessionTokenRef.current);
      }
      socket = new WebSocket(`${getApiWebSocketBaseUrl()}/ws/creator/live?${params.toString()}`);

      socket.addEventListener("open", () => {
        if (!cancelled) {
          setSocketStatus("open");
          setError(null);
        }
      });

      socket.addEventListener("message", (event) => {
        if (cancelled) return;
        try {
          const payload = JSON.parse(String(event.data)) as
            | {
                type?: string;
                sessionToken?: string;
                control?: CreatorLiveControlResponse;
                runtime?: CreatorLiveRuntimeResponse;
              }
            | undefined;
          if (payload?.type === "sessionReady" && payload.sessionToken) {
            socketSessionTokenRef.current = payload.sessionToken;
            return;
          }
          if (payload?.type === "creatorLiveState" && payload.control && payload.runtime) {
            applyLiveState(payload.control, payload.runtime);
          }
        } catch {
          // Ignore malformed frames and keep the connection alive.
        }
      });

      socket.addEventListener("close", () => {
        if (cancelled) return;
        setSocketStatus("closed");
        reconnectTimer = window.setTimeout(connect, 1500);
      });

      socket.addEventListener("error", () => {
        if (!cancelled) {
          setSocketStatus("closed");
        }
      });
    };

    connect();
    return () => {
      cancelled = true;
      socketSessionTokenRef.current = null;
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
      }
      socket?.close();
    };
  }, []);

  const profile: CreatorProfile | null = control?.snapshot.profile ?? null;
  const current = control?.snapshot.currentBroadcast ?? null;
  const pending = control?.snapshot.pendingBroadcast ?? null;
  const activeBroadcast = current ?? pending;
  const isLive = control?.isLive ?? false;
  const previewImage = activeBroadcast?.thumbnail ?? profile?.banner ?? "";
  const moderationStreamId = isLive ? `lv-${profile?.handle ?? ""}-live` : null;
  const viewerHistory = control?.viewerHistory ?? [];
  const bitrateHistory = control?.bitrateHistory ?? [];
  const health = control?.health;
  const scenes = control?.settings.scenes ?? [];
  const collaboration = control?.collaboration;
  const activeCollaborationControl: CreatorCollaborationControlResponse | null =
    collaboration?.activeControl ?? null;
  const activeCollaborationSession = activeCollaborationControl?.runtime.session ?? null;
  const activeCollaborationRuntime = activeCollaborationControl?.runtime ?? null;
  const activeParticipants = activeCollaborationRuntime?.session.participants ?? [];
  const recentCollaborationSessions = collaboration?.recentSessions ?? [];
  const pendingInvites = activeCollaborationSession?.participant
    ? activeCollaborationSession
    : null;
  void pendingInvites;

  const addTag = () => {
    const normalized = tagDraft.trim().toLowerCase();
    if (!normalized || tags.includes(normalized)) return;
    dirtyRef.current = true;
    setTags([...tags, normalized]);
    setTagDraft("");
  };

  const removeTag = (tag: string) => {
    dirtyRef.current = true;
    setTags(tags.filter((item) => item !== tag));
  };

  const copy = async (text: string, which: "key" | "url") => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(which);
      window.setTimeout(() => setCopied(null), 1400);
    } catch {
      // Ignore clipboard availability failures.
    }
  };

  const persistSetup = async () => {
    if (!category) return;
    setSaving(true);
    setError(null);
    try {
      await Promise.all([
        requestJson<unknown>("/api/v1/creator/me/live", {
          method: "PATCH",
          body: {
            title,
            category,
            tags,
            isMature,
          },
        }),
        requestJson<CreatorLiveSettings>("/api/v1/creator/me/live/settings", {
          method: "PATCH",
          body: {
            subscriberOnly,
            slowModeSeconds,
            notifyFollowersDefault: notify,
            activeSceneId: activeScene,
            scenes: scenes.map((scene) => ({
              ...scene,
              active: scene.id === activeScene,
            })),
          },
        }),
      ]);
      dirtyRef.current = false;
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to save live settings.");
    } finally {
      setSaving(false);
    }
  };

  const startBroadcast = async () => {
    if (!title.trim()) {
      setError("Broadcast title is required.");
      return;
    }
    setActionPending("start");
    setError(null);
    try {
      await requestJson<Broadcast>("/api/v1/creator/me/broadcasts/start", {
        method: "POST",
        body: {
          title: title.trim(),
          category,
          tags,
          isMature,
          notifyFollowers: notify,
          thumbnail: previewImage || undefined,
        },
      });
      dirtyRef.current = false;
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to start stream.");
    } finally {
      setActionPending(null);
    }
  };

  const endBroadcast = async () => {
    if (!activeBroadcast) return;
    setActionPending("end");
    setError(null);
    try {
      await requestJson<Broadcast>(`/api/v1/creator/me/broadcasts/${activeBroadcast.id}/end`, {
        method: "POST",
      });
      dirtyRef.current = false;
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to end stream.");
    } finally {
      setActionPending(null);
    }
  };

  const rotateStreamKey = async () => {
    setActionPending("rotate");
    setError(null);
    try {
      await requestJson<CreatorProfile>("/api/v1/creator/me/stream-key/rotate", {
        method: "POST",
      });
      dirtyRef.current = false;
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to rotate stream key.");
    } finally {
      setActionPending(null);
    }
  };

  const createCollaborationSession = async () => {
    setActionPending("collab-create");
    setError(null);
    try {
      await requestJson("/api/v1/creator/me/live/collabs/sessions", {
        method: "POST",
        body: {
          broadcastId: activeBroadcast?.id,
          title: collabTitle.trim() || title.trim() || "Collaboration Session",
          chatMode: collabChatMode,
          recordingPolicy: collabRecordingPolicy,
        },
      });
      await refresh();
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Unable to create collaboration session.",
      );
    } finally {
      setActionPending(null);
    }
  };

  const reconcileCollaborationSession = async () => {
    if (!activeCollaborationSession) return;
    setActionPending("collab-reconcile");
    setError(null);
    try {
      await requestJson(
        `/api/v1/creator/me/live/collabs/sessions/${activeCollaborationSession.id}/reconcile`,
        {
          method: "POST",
        },
      );
      await refresh();
    } catch (nextError) {
      setError(
        nextError instanceof Error ? nextError.message : "Unable to reconcile collaboration.",
      );
    } finally {
      setActionPending(null);
    }
  };

  const endCollaborationSession = async () => {
    if (!activeCollaborationSession) return;
    setActionPending("collab-end");
    setError(null);
    try {
      await requestJson(
        `/api/v1/creator/me/live/collabs/sessions/${activeCollaborationSession.id}/end`,
        {
          method: "POST",
        },
      );
      await refresh();
    } catch (nextError) {
      setError(
        nextError instanceof Error ? nextError.message : "Unable to end collaboration session.",
      );
    } finally {
      setActionPending(null);
    }
  };

  const createCollaborationInvite = async () => {
    if (!activeCollaborationSession) return;
    if (!inviteeUserId.trim()) {
      setError("Invitee user id is required.");
      return;
    }
    setActionPending("collab-invite");
    setError(null);
    try {
      await requestJson(
        `/api/v1/creator/me/live/collabs/sessions/${activeCollaborationSession.id}/invites`,
        {
          method: "POST",
          body: {
            inviteeUserId: inviteeUserId.trim(),
            role: inviteRole,
            mirrorToGuestChannel: inviteMirror,
            message: inviteMessage.trim() || undefined,
            expiresInMinutes: inviteExpiresMinutes,
          },
        },
      );
      setInviteeUserId("");
      setInviteMessage("");
      setInviteMirror(false);
      setInviteRole("guest");
      setInviteExpiresMinutes(30);
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to create invite.");
    } finally {
      setActionPending(null);
    }
  };

  const updateParticipant = async (
    participantId: string,
    body: {
      state?: string;
      publishToHost?: boolean;
      mirrorToGuestChannel?: boolean;
      canSpeakInChat?: boolean;
    },
    pendingKey: string,
  ) => {
    if (!activeCollaborationSession) return;
    setActionPending(pendingKey);
    setError(null);
    try {
      await requestJson(
        `/api/v1/creator/me/live/collabs/sessions/${activeCollaborationSession.id}/participants/${participantId}`,
        {
          method: "PATCH",
          body,
        },
      );
      await refresh();
    } catch (nextError) {
      setError(
        nextError instanceof Error ? nextError.message : "Unable to update participant state.",
      );
    } finally {
      setActionPending(null);
    }
  };

  const removeParticipant = async (participantId: string) => {
    if (!activeCollaborationSession) return;
    setActionPending(`collab-remove-${participantId}`);
    setError(null);
    try {
      await requestJson(
        `/api/v1/creator/me/live/collabs/sessions/${activeCollaborationSession.id}/participants/${participantId}/remove`,
        {
          method: "POST",
        },
      );
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to remove participant.");
    } finally {
      setActionPending(null);
    }
  };

  const issueMirrorGrant = async (participantId: string) => {
    if (!activeCollaborationSession) return;
    setActionPending(`collab-grant-${participantId}`);
    setError(null);
    try {
      await requestJson(
        `/api/v1/creator/me/live/collabs/sessions/${activeCollaborationSession.id}/participants/${participantId}/grants/mirror`,
        {
          method: "POST",
        },
      );
      await refresh();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to issue mirror grant.");
    } finally {
      setActionPending(null);
    }
  };

  useEffect(() => {
    const streamId = moderationStreamId;
    if (!streamId) {
      setModerators([]);
      setModerationActions([]);
      setModerationReports([]);
      setModerationAudit([]);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const [nextModerators, nextActions, nextReports, nextAudit] = await Promise.all([
          requestJson<ReadonlyArray<CreatorModerator>>(
            `/api/v1/live/streams/${streamId}/moderation/moderators`,
          ),
          requestJson<ReadonlyArray<LiveModerationAction>>(
            `/api/v1/live/streams/${streamId}/moderation/actions`,
          ),
          requestJson<ReadonlyArray<LiveStreamReportRecord>>(
            `/api/v1/live/streams/${streamId}/moderation/reports`,
          ),
          requestJson<ReadonlyArray<ModerationAuditEntry>>(
            `/api/v1/live/streams/${streamId}/moderation/audit`,
          ),
        ]);
        if (cancelled) return;
        setModerators(nextModerators);
        setModerationActions(nextActions);
        setModerationReports(nextReports);
        setModerationAudit(nextAudit);
      } catch (nextError) {
        if (!cancelled) {
          setError(
            nextError instanceof Error
              ? nextError.message
              : "Unable to load moderation controls.",
          );
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [moderationStreamId]);

  const refreshModeration = async () => {
    if (!moderationStreamId) return;
    const [nextModerators, nextActions, nextReports, nextAudit] = await Promise.all([
      requestJson<ReadonlyArray<CreatorModerator>>(
        `/api/v1/live/streams/${moderationStreamId}/moderation/moderators`,
      ),
      requestJson<ReadonlyArray<LiveModerationAction>>(
        `/api/v1/live/streams/${moderationStreamId}/moderation/actions`,
      ),
      requestJson<ReadonlyArray<LiveStreamReportRecord>>(
        `/api/v1/live/streams/${moderationStreamId}/moderation/reports`,
      ),
      requestJson<ReadonlyArray<ModerationAuditEntry>>(
        `/api/v1/live/streams/${moderationStreamId}/moderation/audit`,
      ),
    ]);
    setModerators(nextModerators);
    setModerationActions(nextActions);
    setModerationReports(nextReports);
    setModerationAudit(nextAudit);
  };

  const addModerator = async () => {
    if (!moderationStreamId) return;
    if (!moderatorUserId.trim()) {
      setError("Moderator user id is required.");
      return;
    }
    setActionPending("moderator-add");
    setError(null);
    try {
      await requestJson(`/api/v1/live/streams/${moderationStreamId}/moderation/moderators`, {
        method: "POST",
        body: {
          userId: moderatorUserId.trim(),
          role: moderatorRole,
        },
      });
      setModeratorUserId("");
      await refreshModeration();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to add moderator.");
    } finally {
      setActionPending(null);
    }
  };

  const removeModerator = async (userId: string) => {
    if (!moderationStreamId) return;
    setActionPending(`moderator-remove-${userId}`);
    setError(null);
    try {
      await requestJson(
        `/api/v1/live/streams/${moderationStreamId}/moderation/moderators/${userId}`,
        { method: "DELETE" },
      );
      await refreshModeration();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to remove moderator.");
    } finally {
      setActionPending(null);
    }
  };

  const createModerationAction = async () => {
    if (!moderationStreamId) return;
    if (!actionSubjectUserId.trim() || !actionReason.trim()) {
      setError("Moderation subject and reason are required.");
      return;
    }
    setActionPending("moderation-action");
    setError(null);
    try {
      await requestJson(`/api/v1/live/streams/${moderationStreamId}/moderation/actions`, {
        method: "POST",
        body: {
          subjectUserId: actionSubjectUserId.trim(),
          actionType,
          reason: actionReason.trim(),
          durationMinutes: actionDurationMinutes,
        },
      });
      setActionSubjectUserId("");
      setActionReason("");
      setActionDurationMinutes(15);
      await refreshModeration();
    } catch (nextError) {
      setError(
        nextError instanceof Error ? nextError.message : "Unable to create moderation action.",
      );
    } finally {
      setActionPending(null);
    }
  };

  const revokeModerationAction = async (actionId: string) => {
    if (!moderationStreamId) return;
    setActionPending(`moderation-revoke-${actionId}`);
    setError(null);
    try {
      await requestJson(
        `/api/v1/live/streams/${moderationStreamId}/moderation/actions/${actionId}/revoke`,
        { method: "POST" },
      );
      await refreshModeration();
    } catch (nextError) {
      setError(
        nextError instanceof Error ? nextError.message : "Unable to revoke moderation action.",
      );
    } finally {
      setActionPending(null);
    }
  };

  const resolveReport = async (reportId: string) => {
    if (!moderationStreamId) return;
    setActionPending(`report-resolve-${reportId}`);
    setError(null);
    try {
      await requestJson(
        `/api/v1/live/streams/${moderationStreamId}/moderation/reports/${reportId}`,
        {
          method: "PATCH",
          body: {
            status: reportResolutionStatus,
            resolutionNote: reportResolutionNote.trim() || undefined,
          },
        },
      );
      setReportResolutionNote("");
      await refreshModeration();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Unable to resolve report.");
    } finally {
      setActionPending(null);
    }
  };

  if (loading) {
    return (
      <CreatorLayout>
        <div className="ls-cpage">
          <div className="ls-cpage__empty">Loading live control…</div>
        </div>
      </CreatorLayout>
    );
  }

  if (!control || !runtime || !profile) {
    return (
      <CreatorLayout>
        <div className="ls-cpage">
          <div className="ls-cpage__empty">{error ?? "Live control is unavailable."}</div>
        </div>
      </CreatorLayout>
    );
  }

  return (
    <CreatorLayout>
      <div className="ls-cpage">
        <header className="ls-cpage__head">
          <div>
            <h1 className="ls-cpage__title">{isLive ? "You're live" : "Go live"}</h1>
            <p className="ls-cpage__sub">
              Backend-authoritative live control, ingest health, and collaboration state.
            </p>
          </div>
          <div className="ls-clive__actions">
            <Button
              variant="ghost"
              size="lg"
              icon={<RefreshCw />}
              onClick={() => {
                dirtyRef.current = false;
                void refresh();
              }}
            >
              Refresh
            </Button>
            <Button
              variant="outline"
              size="lg"
              onClick={() => void persistSetup()}
              disabled={saving || actionPending !== null}
            >
              {saving ? "Saving…" : "Save setup"}
            </Button>
            {isLive ? (
              <Button
                variant="danger"
                size="lg"
                icon={<Square fill="currentColor" />}
                onClick={() => void endBroadcast()}
                disabled={actionPending !== null}
              >
                {actionPending === "end" ? "Ending…" : "End stream"}
              </Button>
            ) : (
              <Button
                variant="primary"
                size="lg"
                icon={<Radio fill="currentColor" />}
                onClick={() => void startBroadcast()}
                disabled={actionPending !== null}
              >
                {actionPending === "start" ? "Starting…" : "Start stream"}
              </Button>
            )}
            <Badge tone={socketStatus === "open" ? "new" : "premium"}>
              {socketStatus === "open" ? "Realtime connected" : "Realtime reconnecting"}
            </Badge>
          </div>
        </header>

        {error ? <div className="ls-cpage__empty">{error}</div> : null}

        <section className="ls-cpage__stat-grid">
          <StatCard
            label="Live viewers"
            value={formatViewers(control.currentViewers)}
            spark={viewerHistory}
            accent="#ff2d55"
            footer={`${collaboration?.activeSessionCount ?? 0} active collab session${(collaboration?.activeSessionCount ?? 0) === 1 ? "" : "s"}`}
          />
          <StatCard
            label="Uptime"
            value={activeBroadcast?.startedAt ? formatUptime(activeBroadcast.startedAt) : "00:00"}
            accent="#4ea1ff"
            footer={
              activeBroadcast?.startedAt
                ? `Started ${new Date(activeBroadcast.startedAt).toLocaleTimeString("en-US", {
                    hour: "numeric",
                    minute: "2-digit",
                  })}`
                : "No active broadcast"
            }
          />
          <StatCard
            label="Bitrate"
            value={String(health?.currentBitrateKbps ?? 0)}
            unit="kbps"
            spark={bitrateHistory}
            accent="#3dffb5"
            footer={`${health?.currentDroppedFrames ?? 0} dropped frames`}
          />
          <StatCard
            label="Collab grants"
            value={String(collaboration?.activeGrantCount ?? 0)}
            accent="#ffd83d"
            footer={`${collaboration?.pendingInviteCount ?? 0} pending invites`}
          />
        </section>

        <div className="ls-cpage__split">
          <section className="ls-cpage__section">
            <div className="ls-cpage__section-label mono">Stream preview</div>
            <div
              className="ls-clive__preview"
              style={{
                backgroundImage: `url(${previewImage})`,
              }}
            >
              <div className="ls-clive__preview-scrim" />
              <div className="ls-clive__preview-hud">
                <div className="ls-clive__preview-status">
                  <span
                    className={`ls-clive__preview-dot ls-clive__preview-dot--${isLive ? "live" : "off"}`}
                  />
                  {isLive ? "BROADCAST LIVE" : activeBroadcast ? "READY · WAITING FOR INGEST" : "OFFLINE"}
                </div>
                <div className="ls-clive__preview-stats mono">
                  <span>
                    <Wifi size={11} /> {health?.currentBitrateKbps ?? 0} kbps
                  </span>
                  <span>
                    <Cpu size={11} /> {health?.currentCpuPercent ?? 0}%
                  </span>
                  <span>
                    <HardDrive size={11} /> {health?.currentFreeDiskGb ?? 0} GB free
                  </span>
                  <span>
                    <Activity size={11} /> {health?.currentDroppedFrames ?? 0} drops
                  </span>
                </div>
              </div>
              <div className="ls-clive__scenes">
                {scenes.map((scene) => (
                  <button
                    key={scene.id}
                    type="button"
                    className={`ls-clive__scene ${activeScene === scene.id ? "is-active" : ""}`}
                    onClick={() => {
                      dirtyRef.current = true;
                      setActiveScene(scene.id);
                    }}
                  >
                    <span className="ls-clive__scene-dot" />
                    {scene.label}
                  </button>
                ))}
              </div>
            </div>

            <div className="ls-clive__setup">
              <div className="ls-cpage__card-title">Broadcast info</div>
              <label className="ls-clive__field">
                <span className="ls-clive__field-label mono">Title</span>
                <input
                  type="text"
                  value={title}
                  onChange={(event) => {
                    dirtyRef.current = true;
                    setTitle(event.target.value);
                  }}
                  placeholder="e.g. distributed job queue in rust"
                  maxLength={140}
                />
                <span className="ls-clive__field-hint mono">{title.length}/140</span>
              </label>

              <label className="ls-clive__field">
                <span className="ls-clive__field-label mono">Category</span>
                <div className="ls-clive__category">
                  {categoryOptions.map((option) => (
                    <button
                      key={option}
                      type="button"
                      className={`ls-clive__cat-chip ${category === option ? "is-active" : ""}`}
                      onClick={() => {
                        dirtyRef.current = true;
                        setCategory(option);
                      }}
                    >
                      {option}
                    </button>
                  ))}
                </div>
              </label>

              <label className="ls-clive__field">
                <span className="ls-clive__field-label mono">Tags</span>
                <div className="ls-clive__tags">
                  {tags.map((tag) => (
                    <span key={tag} className="ls-clive__tag">
                      {tag}
                      <button
                        type="button"
                        onClick={() => removeTag(tag)}
                        aria-label={`Remove ${tag}`}
                      >
                        ×
                      </button>
                    </span>
                  ))}
                  <input
                    type="text"
                    value={tagDraft}
                    onChange={(event) => {
                      dirtyRef.current = true;
                      setTagDraft(event.target.value);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === ",") {
                        event.preventDefault();
                        addTag();
                      }
                    }}
                    placeholder="add tag and press Enter"
                  />
                </div>
              </label>

              <div className="ls-clive__field ls-clive__field--row">
                <label className="ls-clive__switch">
                  <input
                    type="checkbox"
                    checked={isMature}
                    onChange={(event) => {
                      dirtyRef.current = true;
                      setIsMature(event.target.checked);
                    }}
                  />
                  <span className="ls-clive__switch-track">
                    <span className="ls-clive__switch-dot" />
                  </span>
                  <span>Mature content warning</span>
                </label>
                <label className="ls-clive__switch">
                  <input
                    type="checkbox"
                    checked={notify}
                    onChange={(event) => {
                      dirtyRef.current = true;
                      setNotify(event.target.checked);
                    }}
                  />
                  <span className="ls-clive__switch-track">
                    <span className="ls-clive__switch-dot" />
                  </span>
                  <span>Notify followers when I go live</span>
                </label>
              </div>
            </div>
          </section>

          <aside className="ls-clive__aside">
            <section className="ls-clive__box">
              <div className="ls-cpage__card-title">Stream key · RTMP</div>
              <div className="ls-clive__key">
                <div className="ls-clive__key-label mono">RTMP URL</div>
                <div className="ls-clive__key-value mono">{profile.rtmpUrl}</div>
                <button
                  type="button"
                  className="ls-clive__key-copy"
                  onClick={() => void copy(profile.rtmpUrl, "url")}
                >
                  {copied === "url" ? <Check size={13} /> : <Copy size={13} />}
                </button>
              </div>
              <div className="ls-clive__key">
                <div className="ls-clive__key-label mono">Stream key</div>
                <div className="ls-clive__key-value mono">
                  {showKey ? profile.streamKey : "•".repeat(profile.streamKey.length)}
                </div>
                <button
                  type="button"
                  className="ls-clive__key-copy"
                  onClick={() => setShowKey((value) => !value)}
                  aria-label={showKey ? "Hide key" : "Show key"}
                >
                  {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
                </button>
                <button
                  type="button"
                  className="ls-clive__key-copy"
                  onClick={() => void copy(profile.streamKey, "key")}
                >
                  {copied === "key" ? <Check size={13} /> : <Copy size={13} />}
                </button>
              </div>
              <div className="ls-clive__key-actions">
                <Button
                  variant="ghost"
                  size="sm"
                  icon={<RefreshCw />}
                  onClick={() => void rotateStreamKey()}
                  disabled={actionPending !== null}
                >
                  {actionPending === "rotate" ? "Rotating…" : "Rotate key"}
                </Button>
                <Badge tone="premium">{control.subscriberTiers.length} subscriber tiers</Badge>
              </div>
              <p className="ls-clive__key-note">
                Rotating the stream key immediately invalidates any active ingest encoder.
              </p>
            </section>

            <section className="ls-clive__box">
              <div className="ls-cpage__card-title">Chat & collaboration</div>
              <div className="ls-clive__chat-preview">
                <div className="ls-clive__chat-line mono">
                  <MessageSquare size={12} /> subscribers-only: {subscriberOnly ? "on" : "off"}
                </div>
                <div className="ls-clive__chat-line mono">
                  <Activity size={12} /> slow mode: {slowModeSeconds}s
                </div>
                <div className="ls-clive__chat-line mono">
                  <Users size={12} /> active sessions: {collaboration?.activeSessionCount ?? 0}
                </div>
                <div className="ls-clive__chat-line mono">
                  <Link2 size={12} /> mirror grants: {collaboration?.activeGrantCount ?? 0}
                </div>
                <div className="ls-clive__chat-line mono">
                  pending invites: {collaboration?.pendingInviteCount ?? 0}
                </div>
              </div>

              <div className="ls-clive__chat-settings">
                <label className="ls-clive__switch">
                  <input
                    type="checkbox"
                    checked={subscriberOnly}
                    onChange={(event) => {
                      dirtyRef.current = true;
                      setSubscriberOnly(event.target.checked);
                    }}
                  />
                  <span className="ls-clive__switch-track">
                    <span className="ls-clive__switch-dot" />
                  </span>
                  <span>Subscribers-only</span>
                </label>
                <label className="ls-clive__switch">
                  <input
                    type="checkbox"
                    checked={slowModeSeconds > 0}
                    onChange={(event) => {
                      dirtyRef.current = true;
                      setSlowModeSeconds(event.target.checked ? 3 : 0);
                    }}
                  />
                  <span className="ls-clive__switch-track">
                    <span className="ls-clive__switch-dot" />
                  </span>
                  <span>Slow mode ({slowModeSeconds}s)</span>
                </label>
                <div className="ls-clive__chat-line mono">auto-mod: {control.settings.autoModLevel}</div>
              </div>

              <div className="ls-clive__collab">
                {activeCollaborationControl ? (
                  <>
                    <div className="ls-clive__collab-head">
                      <div>
                        <div className="ls-clive__collab-title">{activeCollaborationSession?.title}</div>
                        <div className="ls-clive__collab-meta mono">
                          {activeCollaborationSession?.status} · chat {activeCollaborationRuntime?.topology.chatMode} · record{" "}
                          {activeCollaborationRuntime?.topology.recordingPolicy}
                        </div>
                      </div>
                      <div className="ls-clive__collab-actions">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => void reconcileCollaborationSession()}
                          disabled={actionPending !== null}
                        >
                          {actionPending === "collab-reconcile" ? "Reconciling…" : "Reconcile"}
                        </Button>
                        <Button
                          variant="danger"
                          size="sm"
                          onClick={() => void endCollaborationSession()}
                          disabled={actionPending !== null}
                        >
                          {actionPending === "collab-end" ? "Ending…" : "End collab"}
                        </Button>
                      </div>
                    </div>

                    <div className="ls-clive__collab-stats">
                      <div className="ls-clive__collab-stat">
                        <span className="mono faint">Connected</span>
                        <strong>{activeCollaborationRuntime?.topology.connectedParticipants ?? 0}</strong>
                      </div>
                      <div className="ls-clive__collab-stat">
                        <span className="mono faint">Pending invites</span>
                        <strong>{activeCollaborationControl.pendingInviteCount}</strong>
                      </div>
                      <div className="ls-clive__collab-stat">
                        <span className="mono faint">Active grants</span>
                        <strong>{activeCollaborationControl.activeGrantCount}</strong>
                      </div>
                      <div className="ls-clive__collab-stat">
                        <span className="mono faint">Stale sockets</span>
                        <strong>{activeCollaborationControl.staleSocketCount}</strong>
                      </div>
                    </div>

                    <div className="ls-clive__collab-panel">
                      <div className="ls-clive__collab-section-title">Invite participant</div>
                      <div className="ls-clive__collab-form">
                        <input
                          type="text"
                          value={inviteeUserId}
                          onChange={(event) => setInviteeUserId(event.target.value)}
                          placeholder="invitee user id"
                        />
                        <select value={inviteRole} onChange={(event) => setInviteRole(event.target.value)}>
                          {collaborationRoleOptions.map((option) => (
                            <option key={option.value} value={option.value}>
                              {option.label}
                            </option>
                          ))}
                        </select>
                        <input
                          type="number"
                          min={5}
                          max={1440}
                          value={inviteExpiresMinutes}
                          onChange={(event) =>
                            setInviteExpiresMinutes(
                              Number.isFinite(event.target.valueAsNumber)
                                ? event.target.valueAsNumber
                                : 30,
                            )
                          }
                          placeholder="expires minutes"
                        />
                        <input
                          type="text"
                          value={inviteMessage}
                          onChange={(event) => setInviteMessage(event.target.value)}
                          placeholder="invite message"
                        />
                        <label className="ls-clive__switch">
                          <input
                            type="checkbox"
                            checked={inviteMirror}
                            onChange={(event) => setInviteMirror(event.target.checked)}
                          />
                          <span className="ls-clive__switch-track">
                            <span className="ls-clive__switch-dot" />
                          </span>
                          <span>Mirror to guest channel</span>
                        </label>
                        <Button
                          variant="primary"
                          size="sm"
                          onClick={() => void createCollaborationInvite()}
                          disabled={actionPending !== null}
                        >
                          {actionPending === "collab-invite" ? "Inviting…" : "Send invite"}
                        </Button>
                      </div>
                    </div>

                    <div className="ls-clive__collab-panel">
                      <div className="ls-clive__collab-section-title">Participants</div>
                      <div className="ls-clive__collab-list">
                        {activeParticipants.map((participant) => {
                          const nextState = participantStateActionTarget(participant.state);
                          const canIssueMirrorGrant =
                            participant.role !== "host" &&
                            participant.state === "live" &&
                            participant.mirrorToGuestChannel;
                          return (
                            <div key={participant.id} className="ls-clive__collab-card">
                              <div className="ls-clive__collab-card-head">
                                <div>
                                  <div className="ls-clive__collab-card-title">
                                    {participant.creatorId ?? participant.userId}
                                  </div>
                                  <div className="ls-clive__collab-card-meta mono">
                                    {participant.role} · {participant.state} · joined{" "}
                                    {formatTimestamp(participant.joinedAt)}
                                  </div>
                                </div>
                                <Badge tone={participant.state === "live" ? "new" : "premium"}>
                                  {participant.state}
                                </Badge>
                              </div>
                              <div className="ls-clive__collab-toggle-row">
                                <button
                                  type="button"
                                  className={`ls-clive__pill ${participant.publishToHost ? "is-active" : ""}`}
                                  onClick={() =>
                                    void updateParticipant(
                                      participant.id,
                                      { publishToHost: !participant.publishToHost },
                                      `collab-host-${participant.id}`,
                                    )
                                  }
                                  disabled={participant.role === "host" || actionPending !== null}
                                >
                                  Host output {participant.publishToHost ? "on" : "off"}
                                </button>
                                <button
                                  type="button"
                                  className={`ls-clive__pill ${participant.canSpeakInChat ? "is-active" : ""}`}
                                  onClick={() =>
                                    void updateParticipant(
                                      participant.id,
                                      { canSpeakInChat: !participant.canSpeakInChat },
                                      `collab-chat-${participant.id}`,
                                    )
                                  }
                                  disabled={participant.role === "host" || actionPending !== null}
                                >
                                  Chat {participant.canSpeakInChat ? "enabled" : "muted"}
                                </button>
                                <button
                                  type="button"
                                  className={`ls-clive__pill ${participant.mirrorToGuestChannel ? "is-active" : ""}`}
                                  onClick={() =>
                                    void updateParticipant(
                                      participant.id,
                                      { mirrorToGuestChannel: !participant.mirrorToGuestChannel },
                                      `collab-mirror-${participant.id}`,
                                    )
                                  }
                                  disabled={
                                    participant.role === "host" ||
                                    !participant.creatorId ||
                                    actionPending !== null
                                  }
                                >
                                  Mirror {participant.mirrorToGuestChannel ? "on" : "off"}
                                </button>
                              </div>
                              <div className="ls-clive__collab-actions">
                                {nextState ? (
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() =>
                                      void updateParticipant(
                                        participant.id,
                                        { state: nextState },
                                        `collab-state-${participant.id}`,
                                      )
                                    }
                                    disabled={participant.role === "host" || actionPending !== null}
                                  >
                                    {actionPending === `collab-state-${participant.id}`
                                      ? "Updating…"
                                      : participantStateActionLabel(participant.state)}
                                  </Button>
                                ) : null}
                                {canIssueMirrorGrant ? (
                                  <Button
                                    variant="outline"
                                    size="sm"
                                    onClick={() => void issueMirrorGrant(participant.id)}
                                    disabled={actionPending !== null}
                                  >
                                    {actionPending === `collab-grant-${participant.id}`
                                      ? "Issuing…"
                                      : "Issue mirror grant"}
                                  </Button>
                                ) : null}
                                {participant.role !== "host" ? (
                                  <Button
                                    variant="danger"
                                    size="sm"
                                    onClick={() => void removeParticipant(participant.id)}
                                    disabled={actionPending !== null}
                                  >
                                    {actionPending === `collab-remove-${participant.id}`
                                      ? "Removing…"
                                      : "Remove"}
                                  </Button>
                                ) : null}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    </div>

                    <div className="ls-clive__collab-panel">
                      <div className="ls-clive__collab-section-title">Runtime authority</div>
                      <div className="ls-clive__collab-runtime-grid mono">
                        <div>Host outputs: {activeCollaborationRuntime?.topology.hostOutputParticipantIds.length ?? 0}</div>
                        <div>Backstage: {activeCollaborationRuntime?.topology.backstageParticipantIds.length ?? 0}</div>
                        <div>Live: {activeCollaborationRuntime?.topology.liveParticipantIds.length ?? 0}</div>
                        <div>Mirrored channels: {activeCollaborationRuntime?.topology.mirroredCreatorIds.length ?? 0}</div>
                        <div>Socket sessions: {activeCollaborationControl.socketSessions.length}</div>
                        <div>Recent events: {activeCollaborationRuntime?.recentEvents.length ?? 0}</div>
                      </div>
                    </div>
                  </>
                ) : (
                  <div className="ls-clive__collab-panel">
                    <div className="ls-clive__collab-section-title">Start a collaboration session</div>
                    <div className="ls-clive__collab-form">
                      <input
                        type="text"
                        value={collabTitle}
                        onChange={(event) => setCollabTitle(event.target.value)}
                        placeholder="session title"
                      />
                      <select
                        value={collabChatMode}
                        onChange={(event) => setCollabChatMode(event.target.value)}
                      >
                        {collaborationChatModeOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <select
                        value={collabRecordingPolicy}
                        onChange={(event) => setCollabRecordingPolicy(event.target.value)}
                      >
                        {collaborationRecordingPolicyOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <Button
                        variant="primary"
                        size="sm"
                        onClick={() => void createCollaborationSession()}
                        disabled={!activeBroadcast || actionPending !== null}
                      >
                        {actionPending === "collab-create" ? "Starting…" : "Start collab session"}
                      </Button>
                    </div>
                    <p className="ls-clive__key-note">
                      Collaboration sessions attach to the current pending or live broadcast. Start the stream setup first if no broadcast authority exists yet.
                    </p>
                  </div>
                )}

                <div className="ls-clive__collab-panel">
                  <div className="ls-clive__collab-section-title">Recent collaboration history</div>
                  <div className="ls-clive__collab-list">
                    {recentCollaborationSessions.slice(0, 4).map((session) => (
                      <div key={session.id} className="ls-clive__collab-card">
                        <div className="ls-clive__collab-card-head">
                          <div>
                            <div className="ls-clive__collab-card-title">{session.title}</div>
                            <div className="ls-clive__collab-card-meta mono">
                              {session.status} · {session.chatMode} · {session.recordingPolicy}
                            </div>
                          </div>
                          <Badge tone={session.status === "active" ? "new" : "premium"}>
                            {session.status}
                          </Badge>
                        </div>
                        <div className="ls-clive__collab-runtime-grid mono">
                          <div>Created: {formatTimestamp(session.createdAt)}</div>
                          <div>Updated: {formatTimestamp(session.updatedAt)}</div>
                          <div>Participants: {session.participants.length}</div>
                          <div>Invites: {session.invites.length}</div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </section>

            <section className="ls-clive__box">
              <div className="ls-cpage__card-title">Moderation & audit</div>
              {moderationStreamId ? (
                <div className="ls-clive__moderation">
                  <div className="ls-clive__moderation-grid">
                    <div className="ls-clive__collab-stat">
                      <span className="mono faint">Moderators</span>
                      <strong>{moderators.length}</strong>
                    </div>
                    <div className="ls-clive__collab-stat">
                      <span className="mono faint">Active actions</span>
                      <strong>{moderationActions.filter((item) => item.state === "active").length}</strong>
                    </div>
                    <div className="ls-clive__collab-stat">
                      <span className="mono faint">Open reports</span>
                      <strong>{moderationReports.filter((item) => item.status !== "resolved" && item.status !== "dismissed").length}</strong>
                    </div>
                    <div className="ls-clive__collab-stat">
                      <span className="mono faint">Audit events</span>
                      <strong>{moderationAudit.length}</strong>
                    </div>
                  </div>

                  <div className="ls-clive__collab-panel">
                    <div className="ls-clive__collab-section-title">Add moderator</div>
                    <div className="ls-clive__collab-form">
                      <input
                        type="text"
                        value={moderatorUserId}
                        onChange={(event) => setModeratorUserId(event.target.value)}
                        placeholder="moderator user id"
                      />
                      <select
                        value={moderatorRole}
                        onChange={(event) => setModeratorRole(event.target.value)}
                      >
                        {moderatorRoleOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <Button
                        variant="primary"
                        size="sm"
                        onClick={() => void addModerator()}
                        disabled={actionPending !== null}
                      >
                        {actionPending === "moderator-add" ? "Adding…" : "Add moderator"}
                      </Button>
                    </div>
                    <div className="ls-clive__collab-list">
                      {moderators.map((moderator) => (
                        <div key={moderator.userId} className="ls-clive__collab-card">
                          <div className="ls-clive__collab-card-head">
                            <div>
                              <div className="ls-clive__collab-card-title">{moderator.userId}</div>
                              <div className="ls-clive__collab-card-meta mono">
                                {moderator.role} · added {formatTimestamp(moderator.createdAt)}
                              </div>
                            </div>
                            <Button
                              variant="danger"
                              size="sm"
                              onClick={() => void removeModerator(moderator.userId)}
                              disabled={actionPending !== null}
                            >
                              {actionPending === `moderator-remove-${moderator.userId}`
                                ? "Removing…"
                                : "Remove"}
                            </Button>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="ls-clive__collab-panel">
                    <div className="ls-clive__collab-section-title">Create moderation action</div>
                    <div className="ls-clive__collab-form">
                      <input
                        type="text"
                        value={actionSubjectUserId}
                        onChange={(event) => setActionSubjectUserId(event.target.value)}
                        placeholder="subject user id"
                      />
                      <select value={actionType} onChange={(event) => setActionType(event.target.value)}>
                        {moderationActionTypeOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <input
                        type="number"
                        min={1}
                        max={43200}
                        value={actionDurationMinutes}
                        onChange={(event) =>
                          setActionDurationMinutes(
                            Number.isFinite(event.target.valueAsNumber)
                              ? event.target.valueAsNumber
                              : 15,
                          )
                        }
                        placeholder="duration minutes"
                      />
                      <input
                        type="text"
                        value={actionReason}
                        onChange={(event) => setActionReason(event.target.value)}
                        placeholder="reason"
                      />
                      <Button
                        variant="primary"
                        size="sm"
                        onClick={() => void createModerationAction()}
                        disabled={actionPending !== null}
                      >
                        {actionPending === "moderation-action" ? "Applying…" : "Apply action"}
                      </Button>
                    </div>
                    <div className="ls-clive__collab-list">
                      {moderationActions.slice(0, 6).map((item) => (
                        <div key={item.id} className="ls-clive__collab-card">
                          <div className="ls-clive__collab-card-head">
                            <div>
                              <div className="ls-clive__collab-card-title">
                                {item.actionType} · {item.subjectUserId}
                              </div>
                              <div className="ls-clive__collab-card-meta mono">
                                {item.state} · {item.reason} · created {formatTimestamp(item.createdAt)}
                              </div>
                            </div>
                            <Badge tone={item.state === "active" ? "new" : "premium"}>
                              {item.state}
                            </Badge>
                          </div>
                          <div className="ls-clive__collab-actions">
                            <div className="ls-clive__collab-card-meta mono">
                              expires {formatTimestamp(item.expiresAt)}
                            </div>
                            {item.state === "active" ? (
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => void revokeModerationAction(item.id)}
                                disabled={actionPending !== null}
                              >
                                {actionPending === `moderation-revoke-${item.id}`
                                  ? "Revoking…"
                                  : "Revoke"}
                              </Button>
                            ) : null}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="ls-clive__collab-panel">
                    <div className="ls-clive__collab-section-title">Resolve reports</div>
                    <div className="ls-clive__collab-form">
                      <select
                        value={reportResolutionStatus}
                        onChange={(event) => setReportResolutionStatus(event.target.value)}
                      >
                        {reportStatusOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <input
                        type="text"
                        value={reportResolutionNote}
                        onChange={(event) => setReportResolutionNote(event.target.value)}
                        placeholder="resolution note"
                      />
                    </div>
                    <div className="ls-clive__collab-list">
                      {moderationReports.slice(0, 6).map((report) => (
                        <div key={report.id} className="ls-clive__collab-card">
                          <div className="ls-clive__collab-card-head">
                            <div>
                              <div className="ls-clive__collab-card-title">
                                {report.reason} · {report.userId}
                              </div>
                              <div className="ls-clive__collab-card-meta mono">
                                {report.status} · filed {formatTimestamp(report.createdAt)}
                              </div>
                            </div>
                            <Badge tone={report.status === "resolved" ? "new" : "premium"}>
                              {report.status}
                            </Badge>
                          </div>
                          {report.details ? (
                            <div className="ls-clive__collab-card-meta">{report.details}</div>
                          ) : null}
                          <div className="ls-clive__collab-actions">
                            <div className="ls-clive__collab-card-meta mono">
                              resolved {formatTimestamp(report.resolvedAt)}
                            </div>
                            {report.status !== "resolved" && report.status !== "dismissed" ? (
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => void resolveReport(report.id)}
                                disabled={actionPending !== null}
                              >
                                {actionPending === `report-resolve-${report.id}`
                                  ? "Saving…"
                                  : "Update report"}
                              </Button>
                            ) : null}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="ls-clive__collab-panel">
                    <div className="ls-clive__collab-section-title">Audit trail</div>
                    <div className="ls-clive__collab-list">
                      {moderationAudit.slice(0, 8).map((entry) => (
                        <div key={entry.id} className="ls-clive__collab-card">
                          <div className="ls-clive__collab-card-head">
                            <div>
                              <div className="ls-clive__collab-card-title">{entry.eventType}</div>
                              <div className="ls-clive__collab-card-meta mono">
                                actor {entry.actorUserId}
                                {entry.subjectUserId ? ` · subject ${entry.subjectUserId}` : ""}
                              </div>
                            </div>
                            <div className="ls-clive__collab-card-meta mono">
                              {formatTimestamp(entry.createdAt)}
                            </div>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              ) : (
                <div className="ls-clive__key-note">
                  Moderation controls activate once the channel is live and the public live stream authority row exists.
                </div>
              )}
            </section>

            <section className="ls-clive__box">
              <div className="ls-cpage__card-title">Health · last samples</div>
              <div className="ls-clive__health">
                <div className="ls-clive__health-row">
                  <span className="mono faint">Bitrate</span>
                  <Sparkline values={bitrateHistory} accent="#3dffb5" width={180} height={28} />
                  <span className="mono">{health?.currentBitrateKbps ?? 0} kbps</span>
                </div>
                <div className="ls-clive__health-row">
                  <span className="mono faint">Viewers</span>
                  <Sparkline values={viewerHistory} accent="#4ea1ff" width={180} height={28} />
                  <span className="mono">
                    {formatViewers(viewerHistory[viewerHistory.length - 1] ?? 0)}
                  </span>
                </div>
                <div className="ls-clive__health-row">
                  <span className="mono faint">Ingest</span>
                  <span className="mono">
                    {runtime.activeSession?.status ?? "offline"} · {runtime.activeSession?.protocol ?? "rtmp"}
                  </span>
                </div>
              </div>
            </section>
          </aside>
        </div>
      </div>
    </CreatorLayout>
  );
}
