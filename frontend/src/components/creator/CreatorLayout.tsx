import { NavLink, useLocation } from "react-router-dom";
import type { ReactNode } from "react";
import {
  LayoutDashboard,
  Radio,
  Film,
  BarChart3,
  Wallet,
} from "lucide-react";
import { repository } from "@/lib/repository";
import { Avatar } from "@/components/ui/Avatar";
import { formatViewers } from "@/lib/format";
import "./CreatorLayout.css";

interface CreatorLayoutProps {
  readonly children: ReactNode;
}

interface CreatorTab {
  readonly to: string;
  readonly label: string;
  readonly Icon: typeof LayoutDashboard;
  readonly end: boolean;
}

const tabs: ReadonlyArray<CreatorTab> = [
  { to: "/creator", label: "Overview", Icon: LayoutDashboard, end: true },
  { to: "/creator/live", label: "Go Live", Icon: Radio, end: false },
  { to: "/creator/content", label: "Content", Icon: Film, end: false },
  { to: "/creator/analytics", label: "Analytics", Icon: BarChart3, end: false },
  { to: "/creator/revenue", label: "Revenue", Icon: Wallet, end: false },
];

export function CreatorLayout({ children }: CreatorLayoutProps) {
  const profile = repository.getCreatorProfile();
  const location = useLocation();
  const currentTab = tabs.find((t) =>
    t.end ? location.pathname === t.to : location.pathname.startsWith(t.to),
  );

  return (
    <div className="ls-creator">
      <header className="ls-creator__bar">
        <div className="ls-creator__identity">
          <Avatar src={profile.avatar} alt={profile.displayName} size={40} live={profile.liveStatus === "live"} />
          <div>
            <div className="ls-creator__kicker mono">
              / creator studio /
              {currentTab ? ` ${currentTab.label.toLowerCase()}` : ""}
            </div>
            <div className="ls-creator__name">
              {profile.displayName}
              <span className="ls-creator__handle">@{profile.handle}</span>
            </div>
          </div>
        </div>

        <div className="ls-creator__status">
          <span
            className={`ls-creator__status-pill ls-creator__status-pill--${profile.liveStatus}`}
          >
            <span className="ls-creator__status-dot" />
            {profile.liveStatus === "live"
              ? "LIVE NOW"
              : profile.liveStatus === "starting"
                ? "STARTING"
                : "OFFLINE"}
          </span>
          <div className="ls-creator__kv mono">
            <span className="faint">FOLLOWERS</span>
            <span>{formatViewers(profile.followers)}</span>
          </div>
          <div className="ls-creator__kv mono">
            <span className="faint">SUBS</span>
            <span>{profile.subscribers.toLocaleString()}</span>
          </div>
          <div className="ls-creator__kv mono">
            <span className="faint">30D VIEWS</span>
            <span>{formatViewers(profile.monthlyViewers)}</span>
          </div>
        </div>
      </header>

      <nav className="ls-creator__tabs">
        {tabs.map(({ to, label, Icon, end }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              `ls-creator__tab ${isActive ? "is-active" : ""}`
            }
          >
            <Icon size={14} strokeWidth={1.75} />
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>

      <div className="ls-creator__body">{children}</div>
    </div>
  );
}
