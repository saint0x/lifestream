import { Link } from "react-router-dom";
import type { LiveStream } from "@/types";
import { Avatar } from "@/components/ui/Avatar";
import { formatViewers, formatUptime } from "@/lib/format";
import "./LiveCard.css";

interface LiveCardProps {
  readonly stream: LiveStream;
}

export function LiveCard({ stream }: LiveCardProps) {
  return (
    <Link to={`/live/${stream.slug}`} className="ls-live-card">
      <div className="ls-live-card__thumb">
        <img src={stream.thumbnail} alt={stream.title} loading="lazy" />
        <div className="ls-live-card__corner ls-live-card__corner--tl">
          <span className="ls-live-card__live">
            <span className="ls-live-card__live-dot" />
            LIVE
          </span>
        </div>
        <div className="ls-live-card__corner ls-live-card__corner--br mono">
          {formatViewers(stream.viewers)} viewers
        </div>
        <div className="ls-live-card__corner ls-live-card__corner--tr mono">
          {formatUptime(stream.startedAt)} uptime
        </div>
      </div>
      <div className="ls-live-card__body">
        <Avatar src={stream.streamer.avatar} alt={stream.streamer.displayName} size={40} live />
        <div className="ls-live-card__meta">
          <div className="ls-live-card__title">{stream.title}</div>
          <div className="ls-live-card__streamer">
            {stream.streamer.displayName}
            {stream.streamer.isPartner && <span className="ls-live-card__check">✓</span>}
          </div>
          <div className="ls-live-card__tags mono">
            <span className="ls-live-card__category">{stream.category}</span>
            {stream.tags.slice(0, 2).map((t) => (
              <span key={t} className="ls-live-card__tag">{t}</span>
            ))}
          </div>
        </div>
      </div>
    </Link>
  );
}
