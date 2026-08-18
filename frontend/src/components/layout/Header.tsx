import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Search, Bell, Settings, Command } from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import type { ContentItem } from "@/types";
import { Avatar } from "@/components/ui/Avatar";
import { Badge } from "@/components/ui/Badge";
import { formatViewers } from "@/lib/format";
import "./Header.css";

export function Header() {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [notifOpen, setNotifOpen] = useState(false);
  const [userOpen, setUserOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const user = useAppStore((s) => s.user);
  const notifications = useAppStore((s) => s.notifications);
  const results: ReadonlyArray<ContentItem> = query ? repository.search(query).slice(0, 8) : [];

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if ((e.key === "k" && (e.metaKey || e.ctrlKey)) || e.key === "/") {
        e.preventDefault();
        inputRef.current?.focus();
        setOpen(true);
      }
      if (e.key === "Escape") {
        setOpen(false);
        setNotifOpen(false);
        setUserOpen(false);
        inputRef.current?.blur();
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, []);

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) {
        setOpen(false);
        setNotifOpen(false);
        setUserOpen(false);
      }
    };
    window.addEventListener("mousedown", handleClick);
    return () => window.removeEventListener("mousedown", handleClick);
  }, []);

  const goToResult = (item: ContentItem) => {
    setOpen(false);
    setQuery("");
    if (item.kind === "series") navigate(`/series/${item.slug}`);
    else if (item.kind === "film") navigate(`/film/${item.slug}`);
    else navigate(`/live/${item.slug}`);
  };

  const submitSearch = () => {
    if (query.trim()) {
      setOpen(false);
      navigate(`/search?q=${encodeURIComponent(query.trim())}`);
    }
  };

  return (
    <header className="ls-header" ref={rootRef}>
      <div className="ls-header__grid">
        <div className="ls-header__crumbs mono">
          <span>lifestream</span>
          <span className="ls-header__crumb-sep">/</span>
          <span>platform</span>
          <span className="ls-header__crumb-sep">/</span>
          <span className="ls-header__crumb-cur">production</span>
        </div>

        <form
          className="ls-header__search-wrap"
          onSubmit={(e) => {
            e.preventDefault();
            submitSearch();
          }}
        >
          <label className={`ls-header__search ${open ? "is-open" : ""}`}>
            <Search size={14} strokeWidth={1.75} />
            <input
              ref={inputRef}
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setOpen(true);
              }}
              onFocus={() => setOpen(true)}
              placeholder="Search titles, streamers, tags…"
              aria-label="Search"
            />
            <span className="ls-header__kbd mono">
              <Command size={10} /> K
            </span>
          </label>
          {open && query && (
            <div className="ls-header__results">
              {results.length === 0 ? (
                <div className="ls-header__results-empty">
                  No results for <span className="mono">{query}</span>
                </div>
              ) : (
                results.map((item) => (
                  <button
                    key={item.id}
                    className="ls-header__result"
                    type="button"
                    onMouseDown={(e) => {
                      e.preventDefault();
                      goToResult(item);
                    }}
                  >
                    <img
                      src={item.kind === "live" ? item.thumbnail : item.images.thumbnail}
                      alt=""
                      className="ls-header__result-img"
                    />
                    <div className="ls-header__result-body">
                      <div className="ls-header__result-title">{item.title}</div>
                      <div className="ls-header__result-meta mono">
                        {item.kind === "live" ? (
                          <>
                            <Badge tone="live">LIVE</Badge>
                            <span>{item.streamer.displayName}</span>
                            <span>·</span>
                            <span>{formatViewers(item.viewers)} watching</span>
                          </>
                        ) : (
                          <>
                            <span>{item.kind === "series" ? "Series" : "Film"}</span>
                            <span>·</span>
                            <span>{item.year}</span>
                            <span>·</span>
                            <span>{item.genres.slice(0, 2).join(" / ")}</span>
                          </>
                        )}
                      </div>
                    </div>
                  </button>
                ))
              )}
            </div>
          )}
        </form>

        <div className="ls-header__actions">
          <button
            className="ls-header__icon-btn"
            aria-label="Notifications"
            type="button"
            onClick={() => setNotifOpen((v) => !v)}
          >
            <Bell size={16} strokeWidth={1.75} />
            {notifications.some((item) => item.readAt === null || item.readAt === undefined) ? (
              <span className="ls-header__dot" />
            ) : null}
          </button>
          {notifOpen && (
            <div className="ls-header__popover ls-header__popover--notif">
              <div className="ls-header__popover-title mono">Notifications</div>
              <div className="ls-header__notif-list">
                {notifications.length === 0 ? (
                  <div className="ls-header__notif">
                    <div>No notifications yet.</div>
                  </div>
                ) : (
                  notifications.slice(0, 6).map((notification) => (
                    <div key={notification.id} className="ls-header__notif">
                      <div
                        className="ls-header__notif-mark"
                        style={{
                          background:
                            notification.channel === "security"
                              ? "#ff7a3d"
                              : notification.channel === "email"
                                ? "#4ea1ff"
                                : "#5ae2a6",
                        }}
                      />
                      <div>
                        {notification.body}
                        <div className="faint mono" style={{ fontSize: 10 }}>
                          {notification.sentAt}
                        </div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          )}
          <button className="ls-header__icon-btn" aria-label="Settings" type="button">
            <Settings size={16} strokeWidth={1.75} />
          </button>
          <button
            className="ls-header__user"
            type="button"
            onClick={() => setUserOpen((v) => !v)}
          >
            <Avatar src={user.avatar} alt={user.displayName} size={28} />
            <div className="ls-header__user-meta">
              <div className="ls-header__user-name">{user.displayName}</div>
              <div className="ls-header__user-tier mono">{user.tier}</div>
            </div>
          </button>
          {userOpen && (
            <div className="ls-header__popover ls-header__popover--user">
              <div className="ls-header__popover-title mono">Account</div>
              <button
                type="button"
                className="ls-header__menu-item"
                onClick={() => {
                  setUserOpen(false);
                  navigate("/profile");
                }}
              >
                Profile
              </button>
              <button
                type="button"
                className="ls-header__menu-item"
                onClick={() => {
                  setUserOpen(false);
                  navigate("/library");
                }}
              >
                Library
              </button>
              <button
                type="button"
                className="ls-header__menu-item"
                onClick={() => {
                  setUserOpen(false);
                  navigate("/watchlist");
                }}
              >
                Watchlist
              </button>
              <div className="ls-header__menu-sep" />
              <button type="button" className="ls-header__menu-item">Preferences</button>
              <button type="button" className="ls-header__menu-item">Sign out</button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
