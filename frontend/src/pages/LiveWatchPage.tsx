import { useEffect, useState } from "react";
import { useParams, Navigate, Link } from "react-router-dom";
import { Heart, Share2 } from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { VideoPlayer } from "@/components/player/VideoPlayer";
import { Avatar } from "@/components/ui/Avatar";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { formatViewers, formatUptime } from "@/lib/format";
import { LiveCard } from "@/components/content/LiveCard";
import { requestJson, resolveApiUrl } from "@/lib/api";
import { preparePlaybackGrantMediaAuthorization } from "@/lib/playback";
import { shareCurrentPage } from "@/lib/share";
import { getVisitorId } from "@/lib/attribution";
import type { LiveStream, PlaybackGrant } from "@/types";
import "./LiveWatchPage.css";

function categorySlug(category: string): string {
  return category
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function LiveWatchPage() {
  const { slug } = useParams<{ slug: string }>();
  const [stream, setStream] = useState(slug && repository.hasState() ? repository.getLiveStreamBySlug(slug) : undefined);
  const [contextLoading, setContextLoading] = useState(true);
  const [contextError, setContextError] = useState<string | null>(null);
  const [otherStreams, setOtherStreams] = useState<ReadonlyArray<LiveStream>>([]);
  const [playbackGrant, setPlaybackGrant] = useState<PlaybackGrant | null>(null);
  const [playbackLoading, setPlaybackLoading] = useState(false);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [shareStatus, setShareStatus] = useState<string | null>(null);
  const streamId = stream?.id;
  const isFollowing = useAppStore((s) =>
    stream ? s.following.has(stream.streamer.id) : false,
  );
  const toggleFollow = useAppStore((s) => s.toggleFollow);

  useEffect(() => {
    if (!slug) {
      setStream(undefined);
      setContextLoading(false);
      setContextError("Live stream not found.");
      return;
    }
    const controller = new AbortController();
    setContextLoading(true);
    setContextError(null);
    void Promise.all([
      repository.fetchLiveStreamBySlug(slug, controller.signal),
      repository.fetchLiveDiscovery({ sort: "viewers", limit: 5 }, controller.signal),
    ])
      .then(([liveStream, discovery]) => {
        setStream(liveStream);
        setOtherStreams(discovery.streams.filter((item) => item.id !== liveStream.id).slice(0, 4));
      })
      .catch((error) => {
        if (controller.signal.aborted) return;
        setStream(undefined);
        setOtherStreams([]);
        setContextError(error instanceof Error ? error.message : "Unable to load this live stream.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setContextLoading(false);
      });
    return () => controller.abort();
  }, [slug]);

  useEffect(() => {
    if (contextLoading || !stream) return;
    if (!stream.playbackSessionUrl) {
      setPlaybackGrant(null);
      setPlaybackError("Live playback is not available for this stream yet.");
      setPlaybackLoading(false);
      return;
    }

    setPlaybackLoading(true);
    setPlaybackError(null);
    const controller = new AbortController();
    void requestJson<PlaybackGrant>(stream.playbackSessionUrl, {
      method: "POST",
      auth: false,
      body: {
        deviceId: getVisitorId(),
        deviceName: "Browser",
        playerVersion: "vanta-web",
      },
      signal: controller.signal,
    })
      .then(async (grant) => {
        await preparePlaybackGrantMediaAuthorization(grant, controller.signal);
        setPlaybackGrant(grant);
      })
      .catch((error) => {
        if (controller.signal.aborted) return;
        setPlaybackGrant(null);
        setPlaybackError(error instanceof Error ? error.message : "Unable to start live playback.");
      })
      .finally(() => {
        if (controller.signal.aborted) return;
        setPlaybackLoading(false);
      });

    return () => controller.abort();
  }, [contextLoading, stream]);

  useEffect(() => {
    if (!streamId) return;
  }, [streamId]);

  if (contextLoading) {
    return <div className="ls-live-watch__route-state mono">Loading live stream…</div>;
  }

  if (contextError) {
    return <div className="ls-live-watch__route-state ls-live-watch__state--error">{contextError}</div>;
  }

  if (!stream) return <Navigate to="/live" replace />;

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
            poster={playbackGrant?.posterUrl ? resolveApiUrl(playbackGrant.posterUrl) : stream.thumbnail}
            title={stream.title}
            durationSec={72000}
            initialProgressSec={Math.floor(
              (Date.now() - new Date(stream.startedAt).getTime()) / 1000,
            )}
            sourceUrl={playbackGrant ? resolveApiUrl(playbackGrant.manifestUrl) : null}
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
                <Link to={`/category/${categorySlug(stream.category)}`} className="ls-live-watch__cat">
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
              variant="ghost"
              icon={<Share2 />}
              onClick={() => {
                void shareCurrentPage(stream.title)
                  .then(setShareStatus)
                  .catch(() => setShareStatus("Unable to share this stream."));
              }}
            >
              Share
            </Button>
          </div>

          {shareStatus ? <div className="ls-live-watch__status">{shareStatus}</div> : null}

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
            {otherStreams.map((s) => (
              <LiveCard key={s.id} stream={s} />
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
