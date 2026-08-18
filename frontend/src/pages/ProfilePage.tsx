import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { Avatar } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import "./ProfilePage.css";

export function ProfilePage() {
  const user = useAppStore((s) => s.user);
  const profile = useAppStore((s) => s.profile);
  const settings = useAppStore((s) => s.settings);
  const plan = useAppStore((s) => s.plan);
  const watchlistCount = useAppStore((s) => s.watchlistDetails.totalTitles);
  const followingCount = useAppStore((s) => s.followingFeed.totalFollowedStreamers);
  const continueCount = useAppStore((s) => s.library.continueWatching.length);

  return (
    <div className="ls-profile">
      <header className="ls-profile__head">
        <Avatar src={user.avatar} alt={user.displayName} size={80} />
        <div>
          <div className="ls-profile__kicker mono">/ yours / profile</div>
          <h1 className="ls-profile__name">{user.displayName}</h1>
          <div className="ls-profile__meta mono">
            <Badge tone="premium">{user.tier.toUpperCase()}</Badge>
            <span>@{user.handle}</span>
            <span>·</span>
            <span>joined {user.joinedAt}</span>
          </div>
        </div>
        <div className="ls-profile__actions">
          <Button variant="outline">Edit profile</Button>
          <Button variant="ghost">Sign out</Button>
        </div>
      </header>

      <div className="ls-profile__stats">
        <div className="ls-profile__stat">
          <div className="ls-profile__stat-num">{watchlistCount}</div>
          <div className="ls-profile__stat-label mono">On watchlist</div>
        </div>
        <div className="ls-profile__stat">
          <div className="ls-profile__stat-num">{followingCount}</div>
          <div className="ls-profile__stat-label mono">Following</div>
        </div>
        <div className="ls-profile__stat">
          <div className="ls-profile__stat-num">{continueCount}</div>
          <div className="ls-profile__stat-label mono">In library</div>
        </div>
        <div className="ls-profile__stat">
          <div className="ls-profile__stat-num">{profile.hoursWatched}</div>
          <div className="ls-profile__stat-label mono">Hours watched</div>
        </div>
      </div>

      <div className="ls-profile__sections">
        <section className="ls-profile__section">
          <div className="ls-list__label mono">Preferences</div>
          <ul className="ls-profile__prefs">
            <li>
              <span>Mature content</span>
              <span className="ls-profile__value">
                {profile.matureContentAllowed ? "Allowed" : "Restricted"}
              </span>
            </li>
            <li>
              <span>Default audio</span>
              <span className="ls-profile__value">{profile.defaultAudio}</span>
            </li>
            <li>
              <span>Subtitle preset</span>
              <span className="ls-profile__value">{profile.subtitlePreset}</span>
            </li>
            <li>
              <span>Autoplay trailers</span>
              <span className="ls-profile__value">
                {settings.playback.autoplayTrailers ? "On" : "Off"}
              </span>
            </li>
            <li>
              <span>Live chat filter</span>
              <span className="ls-profile__value">{profile.liveChatFilter}</span>
            </li>
          </ul>
        </section>

        <section className="ls-profile__section">
          <div className="ls-list__label mono">Plan · {user.tier}</div>
          <div className="ls-profile__plan">
            <div>
              <div className="ls-profile__plan-title">{plan.planName}</div>
              <div className="ls-profile__plan-meta mono">
                {plan.features.join(" · ")}
              </div>
            </div>
            <Button variant="outline">Manage plan</Button>
          </div>
        </section>
      </div>
    </div>
  );
}
