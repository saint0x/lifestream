import { NavLink } from "react-router-dom";
import { useEffect, useState } from "react";
import {
  Home,
  Radio,
  Film,
  Tv,
  Bookmark,
  Users,
  Library,
  Settings,
  UploadCloud,
  BadgeDollarSign,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { useAppStore } from "@/lib/store";
import { isSignedInUser } from "@/lib/authState";
import { Avatar } from "@/components/ui/Avatar";
import { formatViewers } from "@/lib/format";
import "./Sidebar.css";

const SIDEBAR_COLLAPSED_KEY = "vanta.sidebarCollapsed";

const primary = [
  { to: "/", label: "Home", Icon: Home, end: true },
  { to: "/live", label: "Live", Icon: Radio, end: false },
  { to: "/series", label: "Series", Icon: Tv, end: false },
  { to: "/films", label: "Films", Icon: Film, end: false },
] as const;

const secondary = [
  { to: "/watchlist", label: "Watchlist", Icon: Bookmark },
  { to: "/library", label: "Library", Icon: Library },
  { to: "/following", label: "Following", Icon: Users },
] as const;

const studio = [
  { to: "/studio", label: "Creator Studio", Icon: UploadCloud },
  { to: "/ad-hub", label: "Ad Hub", Icon: BadgeDollarSign },
] as const;

const utility = [
  { to: "/settings", label: "Settings", Icon: Settings },
] as const;

export function Sidebar() {
  const followedLive = useAppStore((s) => s.followingFeed.liveStreams);
  const user = useAppStore((s) => s.user);
  const showCreatorTools = isSignedInUser(user);
  const [collapsed, setCollapsed] = useState(() => {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "true";
  });

  useEffect(() => {
    window.localStorage.setItem(SIDEBAR_COLLAPSED_KEY, String(collapsed));
  }, [collapsed]);

  return (
    <aside className={`ls-sidebar ${collapsed ? "is-collapsed" : ""}`}>
      <div className="ls-sidebar__brand">
        <NavLink to="/" className="ls-sidebar__logo" aria-label="VANTA home">
          <span className="ls-sidebar__logo-mark" />
          <span className="ls-sidebar__logo-text">VANTA</span>
        </NavLink>
        <button
          className="ls-sidebar__collapse"
          type="button"
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!collapsed}
          onClick={() => setCollapsed((value) => !value)}
        >
          {collapsed ? (
            <PanelLeftOpen size={16} strokeWidth={1.8} />
          ) : (
            <PanelLeftClose size={16} strokeWidth={1.8} />
          )}
        </button>
        <div className="ls-sidebar__wordmark mono">
          <span>v0.1.0</span>
          <span className="ls-sidebar__dot" />
          <span>live</span>
        </div>
      </div>

      <nav className="ls-sidebar__section">
        <div className="ls-sidebar__label mono">Browse</div>
        {primary.map(({ to, label, Icon, end }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              `ls-sidebar__item ${isActive ? "is-active" : ""}`
            }
            title={label}
          >
            <Icon size={16} strokeWidth={1.75} />
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>

      <nav className="ls-sidebar__section">
        <div className="ls-sidebar__label mono">Yours</div>
        {secondary.map(({ to, label, Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `ls-sidebar__item ${isActive ? "is-active" : ""}`
            }
            title={label}
          >
            <Icon size={16} strokeWidth={1.75} />
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>

      {showCreatorTools ? (
        <nav className="ls-sidebar__section">
          <div className="ls-sidebar__label mono">Studio</div>
          {studio.map(({ to, label, Icon }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                `ls-sidebar__item ${isActive ? "is-active" : ""}`
              }
              title={label}
            >
              <Icon size={16} strokeWidth={1.75} />
              <span>{label}</span>
            </NavLink>
          ))}
        </nav>
      ) : null}

      {followedLive.length > 0 && (
        <nav className="ls-sidebar__section ls-sidebar__section--followed">
          <div className="ls-sidebar__label mono">Followed · Live</div>
          {followedLive.map((stream) => {
            const streamer = stream.streamer;
            return (
              <NavLink
                key={stream.id}
                to={`/live/${stream.slug}`}
                className={({ isActive }) =>
                  `ls-sidebar__streamer ${isActive ? "is-active" : ""}`
                }
                title={`${streamer.displayName} live`}
              >
                <Avatar src={streamer.avatar} alt={streamer.displayName} size={24} live />
                <span className="ls-sidebar__streamer-name">{streamer.displayName}</span>
                <span className="ls-sidebar__streamer-viewers mono">
                  <span className="ls-sidebar__live-dot" />
                  {formatViewers(stream.viewers)}
                </span>
              </NavLink>
            );
          })}
        </nav>
      )}

      <nav className="ls-sidebar__section ls-sidebar__section--utility">
        <div className="ls-sidebar__label mono">Account</div>
        {utility.map(({ to, label, Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `ls-sidebar__item ${isActive ? "is-active" : ""}`
            }
            title={label}
          >
            <Icon size={16} strokeWidth={1.75} />
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>

      <div className="ls-sidebar__footer mono">
        <span>© VANTA</span>
        <span>/ 2026</span>
      </div>
    </aside>
  );
}
