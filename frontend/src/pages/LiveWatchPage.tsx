import { useEffect, useState } from "react";
import { useParams, Navigate, Link } from "react-router-dom";
import { Heart, Bell, Share2, Gift, Bookmark, Flag } from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { VideoPlayer } from "@/components/player/VideoPlayer";
import { LiveChat } from "@/components/chat/LiveChat";
import { Avatar } from "@/components/ui/Avatar";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { formatViewers, formatUptime } from "@/lib/format";
import { LiveCard } from "@/components/content/LiveCard";
import { getApiBaseUrl, requestJson } from "@/lib/api";
import type { LiveNotifyPreference, PlaybackGrant } from "@/types";
import "./LiveWatchPage.css";

export function LiveWatchPage() {
  const { slug } = useParams<{ slug: string }>();
  const stream = slug ? repository.getLiveStreamBySlug(slug) : undefined;
  const [playbackGrant, setPlaybackGrant] = useState<PlaybackGrant | null>(null);
  const [playbackLoading, setPlaybackLoading] = useState(false);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [notifyEnabled, setNotifyEnabled] = useState(false);
  const [notifyPending, setNotifyPending] = useState(false);
  const [clipPending, setClipPending] = useState(false);
  const [clipStatus, setClipStatus] = useState<string | null>(null);
  const [reportOpen, setReportOpen] = useState(false);
  const [reportReason, setReportReason] = useState("");
  const [reportDetails, setReportDetails] = useState("");
  const [reportPending, setReportPending] = useState(false);
  const [reportStatus, setReportStatus] = useState<string | null>(null);
  const isFollowing = useAppStore((s) =>
    stream ? s.following.has(stream.streamer.id) : false,
  );
  const toggleFollow = useAppStore((s) => s.toggleFollow);

  if (!stream) return <Navigate to="/live" replace />;

  useEffect(() => {
    if (!stream.playbackSessionUrl) {
      setPlaybackGrant(null);
      setPlaybackError("Live playback is not available for this stream yet.");
      setPlaybackLoading(false);
      return;
    }

    setPlaybackLoading(true);
    setPlaybackError(null);
    void requestJson<PlaybackGrant>(stream.playbackSessionUrl, { method: "POST", auth: false })
      .then((grant) => {
        setPlaybackGrant(grant);
      })
      .catch((error) => {
        setPlaybackGrant(null);
        setPlaybackError(error instanceof Error ? error.message : "Unable to start live playback.");
      })
      .finally(() => {
        setPlaybackLoading(false);
      });
  }, [stream.id, stream.playbackSessionUrl]);

  useEffect(() => {
    setNotifyEnabled(false);
    setNotifyPending(false);
    setClipPending(false);
    setClipStatus(null);
    setReportOpen(false);
    setReportReason("");
    setReportDetails("");
    setReportPending(false);
    setReportStatus(null);
  }, [stream.id]);

  const enableNotify = async () => {
    setNotifyPending(true);
    setReportStatus(null);
    try {
      const preference = await requestJson<LiveNotifyPreference>(
        `/api/v1/live/streams/${stream.id}/notify`,
        { method: "POST" },
      );
      setNotifyEnabled(preference.enabled);
    } catch (error) {
      setReportStatus(error instanceof Error ? error.message : "Unable to enable notifications.");
    } finally {
      setNotifyPending(false);
    }
  };

  const createClip = async () => {
    setClipPending(true);
    setClipStatus(null);
    try {
      await requestJson<unknown>(`/api/v1/live/streams/${stream.id}/clip`, {
        method: "POST",
      });
      setClipStatus("Clip request queued for the live stream.");
    } catch (error) {
      setClipStatus(error instanceof Error ? error.message : "Unable to request a clip.");
    } finally {
      setClipPending(false);
    }
  };

  const submitReport = async () => {
    if (!reportReason.trim()) {
      setReportStatus("A report reason is required.");
      return;
    }
    setReportPending(true);
    setReportStatus(null);
    try {
      await requestJson<unknown>(`/api/v1/live/streams/${stream.id}/report`, {
        method: "POST",
        body: {
          reason: reportReason.trim(),
          details: reportDetails.trim() || undefined,
        },
      });
      setReportStatus("Report submitted to the moderation team.");
      setReportReason("");
      setReportDetails("");
      setReportOpen(false);
    } catch (error) {
      setReportStatus(error instanceof Error ? error.message : "Unable to submit report.");
    } finally {
      setReportPending(false);
    }
  };

  const others = repository
    .listLiveStreams()
    .filter((s) => s.id !== stream.id)
    .slice(0, 4);

  return (
    <div className="ls-live-watch">
      <div className="ls-live-watch__main">
        <div className="ls-live-watch__player">
          {playbackLoading ? (
            <div className="ls-live-watch__state">Preparing live playback session…</div>
          ) : null}
          {playbackError ? (
            <div className="ls-live-watch__state ls-live-watch__state--error">{playbackError}</div>
          ) : null}
          <VideoPlayer
            poster={playbackGrant?.posterUrl ? `${getApiBaseUrl()}${playbackGrant.posterUrl}` : stream.thumbnail}
            title={stream.title}
            durationSec={72000}
            initialProgressSec={Math.floor(
              (Date.now() - new Date(stream.startedAt).getTime()) / 1000,
            )}
            sourceUrl={playbackGrant ? `${getApiBaseUrl()}${playbackGrant.manifestUrl}` : null}
            allowPreviewTransport={false}
          />
        </div>

        <div className="ls-live-watch__info">
          <div className="ls-live-watch__streamer">
            <Avatar src={stream.streamer.avatar} alt={stream.streamer.displayName} size={64} live />
            <div>
              <div className="ls-live-watch__name-row">
                <h1 className="ls-live-watch__name">{stream.streamer.displayName}</h1>
                {stream.streamer.isPartner && <Badge tone="new">PARTNER</Badge>}
              </div>
              <div className="ls-live-watch__title">{stream.title}</div>
              <div className="ls-live-watch__tags mono">
                <Link to={`/category/${stream.category.toLowerCase()}`} className="ls-live-watch__cat">
                  {stream.category}
                </Link>
                {stream.tags.map((t) => (
                  <span key={t} className="ls-live-watch__tag">{t}</span>
                ))}
              </div>
            </div>
          </div>

          <div className="ls-live-watch__actions">
            <Button
              variant={isFollowing ? "secondary" : "primary"}
              icon={<Heart fill={isFollowing ? "currentColor" : "none"} />}
              onClick={() => toggleFollow(stream.streamer.id)}
            >
              {isFollowing ? "Following" : "Follow"}
            </Button>
            <Button
              variant={notifyEnabled ? "secondary" : "outline"}
              icon={<Bell />}
              onClick={() => void enableNotify()}
              disabled={notifyPending || notifyEnabled}
            >
              {notifyEnabled ? "Notified" : notifyPending ? "Saving…" : "Notify"}
            </Button>
            <Button variant="outline" icon={<Gift />}>Subscribe</Button>
            <Button variant="ghost" icon={<Share2 />}>Share</Button>
            <Button
              variant="ghost"
              icon={<Bookmark />}
              onClick={() => void createClip()}
              disabled={clipPending}
            >
              {clipPending ? "Clipping…" : "Clip"}
            </Button>
            <Button
              variant={reportOpen ? "secondary" : "ghost"}
              icon={<Flag />}
              aria-label="Report"
              onClick={() => {
                setReportStatus(null);
                setReportOpen((current) => !current);
              }}
            >
              Report
            </Button>
          </div>

          {clipStatus ? <div className="ls-live-watch__status">{clipStatus}</div> : null}
          {reportStatus ? <div className="ls-live-watch__status">{reportStatus}</div> : null}
          {reportOpen ? (
            <div className="ls-live-watch__report">
              <div className="ls-live-watch__section-label mono">Report live stream</div>
              <input
                type="text"
                value={reportReason}
                onChange={(event) => setReportReason(event.target.value)}
                placeholder="Reason"
              />
              <textarea
                value={reportDetails}
                onChange={(event) => setReportDetails(event.target.value)}
                placeholder="Add details for moderation"
                rows={4}
              />
              <div className="ls-live-watch__report-actions">
                <Button
                  variant="outline"
                  onClick={() => {
                    setReportOpen(false);
                    setReportStatus(null);
                  }}
                  disabled={reportPending}
                >
                  Cancel
                </Button>
                <Button
                  variant="danger"
                  onClick={() => void submitReport()}
                  disabled={reportPending}
                >
                  {reportPending ? "Submitting…" : "Submit report"}
                </Button>
              </div>
            </div>
          ) : null}

          <div className="ls-live-watch__stats mono">
            <div className="ls-live-watch__stat">
              <span className="ls-live-watch__stat-dot" />
              <strong>{formatViewers(stream.viewers)}</strong> watching
            </div>
            <div className="ls-live-watch__stat">
              Uptime <strong>{formatUptime(stream.startedAt)}</strong>
            </div>
            <div className="ls-live-watch__stat">
              {stream.streamer.followers.toLocaleString()} followers
            </div>
          </div>

          <div className="ls-live-watch__bio">
            <div className="ls-live-watch__section-label mono">About</div>
            <p>{stream.streamer.bio}</p>
          </div>
        </div>

        <section className="ls-live-watch__recs">
          <div className="ls-live-watch__section-label mono">Also live</div>
          <div className="ls-live-watch__rec-grid">
            {others.map((s) => (
              <LiveCard key={s.id} stream={s} />
            ))}
          </div>
        </section>
      </div>

      <LiveChat streamId={stream.id} streamTitle={stream.title} viewerCount={stream.viewers} />
    </div>
  );
}
