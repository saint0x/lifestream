import { Link } from "react-router-dom";
import { useAppStore } from "@/lib/store";
import { repository } from "@/lib/repository";
import { Avatar } from "@/components/ui/Avatar";
import { Button } from "@/components/ui/Button";
import { formatViewers } from "@/lib/format";
import { LiveCard } from "@/components/content/LiveCard";
import { Users } from "lucide-react";
import "./ListPage.css";

export function FollowingPage() {
  const followingFeed = useAppStore((s) => s.followingFeed);
  const toggleFollow = useAppStore((s) => s.toggleFollow);
  const followedStreamers = followingFeed.followedStreamers;
  const followedLiveStreams = followingFeed.liveStreams;

  return (
    <div className="ls-list">
      <header className="ls-list__head">
        <div className="ls-list__kicker mono">/ yours / following</div>
        <h1 className="ls-list__title">Following</h1>
        <p className="ls-list__sub">
          {followedStreamers.length} streamer{followedStreamers.length === 1 ? "" : "s"} you follow ·
          {" "}{followedLiveStreams.length} live now
        </p>
      </header>

      {followedStreamers.length === 0 ? (
        <div className="ls-list__empty">
          <Users size={24} strokeWidth={1.5} />
          <div>You're not following anyone yet.</div>
          <p>Open a live stream and tap Follow to get notified when someone goes live.</p>
        </div>
      ) : (
        <>
          {followedLiveStreams.length > 0 && (
            <section className="ls-follow__section">
              <div className="ls-list__label mono">Live now</div>
              <div className="ls-follow__live-grid">
                {followedLiveStreams.map((s) => (
                  <LiveCard key={s.id} stream={s} />
                ))}
              </div>
            </section>
          )}

          <section className="ls-follow__section">
            <div className="ls-list__label mono">All followed</div>
            <div className="ls-follow__streamers">
                {followedStreamers.map((s) => {
                const stream = followedLiveStreams.find((x) => x.streamer.id === s.id);
                return (
                  <div key={s.id} className="ls-follow__card">
                    <Link
                      to={stream ? `/live/${stream.slug}` : `/`}
                      className="ls-follow__card-main"
                    >
                      <Avatar src={s.avatar} alt={s.displayName} size={48} live={s.isLive} />
                      <div>
                        <div className="ls-follow__name">
                          {s.displayName}
                          {s.isPartner && <span className="ls-follow__check">✓</span>}
                        </div>
                        <div className="ls-follow__bio">{s.bio}</div>
                        <div className="ls-follow__meta mono">
                          {s.followers.toLocaleString()} followers
                          {stream && (
                            <>
                              <span className="ls-follow__sep">·</span>
                              <span className="ls-follow__live-dot" />
                              {formatViewers(stream.viewers)} watching {stream.category}
                            </>
                          )}
                        </div>
                      </div>
                    </Link>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => toggleFollow(s.id)}
                    >
                      Unfollow
                    </Button>
                  </div>
                );
              })}
            </div>
          </section>
        </>
      )}
    </div>
  );
}
