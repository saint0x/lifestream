import { useEffect, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import {
  Search,
  Bell,
  Settings,
  Command,
  Mail,
  LockKeyhole,
  User,
  X,
  Globe,
  LogIn,
  UserPlus,
  ChevronRight,
} from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { isSignedInUser } from "@/lib/authState";
import { signInWithEmail, signUpWithEmail, startGoogleSignIn } from "@/lib/api";
import type { SearchResult } from "@/types";
import { Avatar } from "@/components/ui/Avatar";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import "./Header.css";

interface Breadcrumb {
  readonly label: string;
  readonly href?: string;
}

function titleize(value: string): string {
  return decodeURIComponent(value)
    .replace(/^@/, "")
    .replace(/[-_]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function findEpisodeContext(id: string) {
  if (!repository.hasState()) return null;
  const episode = repository.getEpisode(id);
  if (!episode) return null;
  const series = repository.getSeriesById(episode.seriesId);
  if (!series) return null;
  return { episode, series };
}

function buildBreadcrumbs(pathname: string, search: string): ReadonlyArray<Breadcrumb> {
  if (pathname === "/") return [{ label: "Home" }];

  const parts = pathname.split("/").filter(Boolean);
  const [section, value, next] = parts;
  const home = { label: "Home", href: "/" };

  if (section?.startsWith("@")) {
    return [home, { label: "Profiles" }, { label: titleize(section) }];
  }

  if (section === "series") {
    const series = value && repository.hasState() ? repository.getSeriesBySlug(value) : undefined;
    return value
      ? [home, { label: "Series", href: "/series" }, { label: series?.title ?? titleize(value) }]
      : [home, { label: "Series" }];
  }

  if (section === "films") return [home, { label: "Films" }];

  if (section === "film") {
    const film = value && repository.hasState() ? repository.getFilmBySlug(value) : undefined;
    return [home, { label: "Films", href: "/films" }, { label: film?.title ?? titleize(value ?? "Film") }];
  }

  if (section === "watch" && value === "episode" && next) {
    const context = findEpisodeContext(next);
    if (context) {
      return [
        home,
        { label: "Series", href: "/series" },
        { label: context.series.title, href: `/series/${context.series.slug}` },
        { label: context.episode.title },
      ];
    }
    return [home, { label: "Watch" }, { label: "Episode" }];
  }

  if (section === "watch" && value === "film" && next) {
    const film = repository.hasState() ? repository.getFilmById(next) : undefined;
    return [home, { label: "Films", href: "/films" }, { label: film?.title ?? "Film" }];
  }

  if (section === "live") {
    const stream = value && repository.hasState() ? repository.getLiveStreamBySlug(value) : undefined;
    return value
      ? [home, { label: "Live", href: "/live" }, { label: stream?.title ?? titleize(value) }]
      : [home, { label: "Live" }];
  }

  if (section === "category") {
    return [home, { label: "Live", href: "/live" }, { label: titleize(value ?? "Category") }];
  }

  if (section === "search") {
    const params = new URLSearchParams(search);
    const query = params.get("q")?.trim();
    return [home, { label: "Search", href: "/search" }, ...(query ? [{ label: query }] : [])];
  }

  if (section === "originals") return [home, { label: "Originals" }];
  if (section === "watchlist") return [home, { label: "Watchlist" }];
  if (section === "library") return [home, { label: "Library" }];
  if (section === "following") return [home, { label: "Following" }];
  if (section === "profile") return [home, { label: "Profile" }];
  if (section === "settings") return [home, { label: "Settings" }];
  if (section === "studio" && value === "tool") {
    return [home, { label: "Creator Studio", href: "/studio" }, { label: titleize(next ?? "Tool") }];
  }
  if (section === "studio") return [home, { label: "Creator Studio" }];
  if (section === "ad-hub") return [home, { label: "Ad Hub" }];

  return [home, { label: titleize(section ?? "Page") }];
}

function BreadcrumbTrail() {
  const location = useLocation();
  const crumbs = buildBreadcrumbs(location.pathname, location.search);

  return (
    <nav className="ls-header__crumbs" aria-label="Page path">
      {crumbs.map((crumb, index) => {
        const isLast = index === crumbs.length - 1;
        return (
          <span className="ls-header__crumb" key={`${crumb.label}-${index}`}>
            {index > 0 ? (
              <ChevronRight
                className="ls-header__crumb-sep"
                size={12}
                strokeWidth={1.8}
                aria-hidden="true"
              />
            ) : null}
            {crumb.href && !isLast ? (
              <Link to={crumb.href}>{crumb.label}</Link>
            ) : (
              <span className="ls-header__crumb-cur" aria-current={isLast ? "page" : undefined}>
                {crumb.label}
              </span>
            )}
          </span>
        );
      })}
    </nav>
  );
}

export function Header() {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [notifOpen, setNotifOpen] = useState(false);
  const [userOpen, setUserOpen] = useState(false);
  const [authOpen, setAuthOpen] = useState(false);
  const [authMode, setAuthMode] = useState<"sign-in" | "sign-up">("sign-in");
  const [authEmail, setAuthEmail] = useState("");
  const [authPassword, setAuthPassword] = useState("");
  const [authDisplayName, setAuthDisplayName] = useState("");
  const [authPending, setAuthPending] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const user = useAppStore((s) => s.user);
  const notifications = useAppStore((s) => s.notifications);
  const signOut = useAppStore((s) => s.signOut);
  const hydrate = useAppStore((s) => s.hydrate);
  const markNotificationRead = useAppStore((s) => s.markNotificationRead);
  const signedIn = isSignedInUser(user);
  const [results, setResults] = useState<ReadonlyArray<SearchResult>>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

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
        setAuthOpen(false);
        inputRef.current?.blur();
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, []);

  useEffect(() => {
    const trimmed = query.trim();
    if (!trimmed) {
      setResults([]);
      setSearchLoading(false);
      setSearchError(null);
      return;
    }
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      setSearchLoading(true);
      setSearchError(null);
      void repository
        .searchRemote(trimmed, controller.signal)
        .then((items) => setResults(items.slice(0, 8)))
        .catch((error) => {
          if (!controller.signal.aborted) {
            setResults([]);
            setSearchError(
              error instanceof Error ? error.message : "Search failed.",
            );
          }
        })
        .finally(() => {
          if (!controller.signal.aborted) setSearchLoading(false);
        });
    }, 180);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [query]);

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

  const goToResult = (item: SearchResult) => {
    setOpen(false);
    setQuery("");
    navigate(item.href);
  };

  const searchKindLabel = (kind: SearchResult["kind"]) => {
    switch (kind) {
      case "series":
        return "Series";
      case "film":
        return "Film";
      case "live":
        return "Live";
      case "episode":
        return "Episode";
      case "creator":
        return "Creator";
      case "profile":
        return "Profile";
      case "category":
        return "Category";
      default:
        return "Result";
    }
  };

  const submitSearch = () => {
    if (query.trim()) {
      setOpen(false);
      navigate(`/search?q=${encodeURIComponent(query.trim())}`);
    }
  };

  const openAuth = (mode: "sign-in" | "sign-up") => {
    setAuthMode(mode);
    setAuthError(null);
    setAuthOpen(true);
    setUserOpen(false);
  };

  const submitAuth = async () => {
    setAuthPending(true);
    setAuthError(null);
    try {
      if (authMode === "sign-up") {
        await signUpWithEmail({
          email: authEmail,
          password: authPassword,
          displayName: authDisplayName || undefined,
        });
      } else {
        await signInWithEmail({ email: authEmail, password: authPassword });
      }
      setAuthOpen(false);
      setAuthEmail("");
      setAuthPassword("");
      setAuthDisplayName("");
      await hydrate();
    } catch (error) {
      setAuthError(
        error instanceof Error ? error.message : "Unable to continue.",
      );
    } finally {
      setAuthPending(false);
    }
  };

  return (
    <header className="ls-header" ref={rootRef}>
      <div className="ls-header__grid">
        <div className="ls-header__nav">
          <BreadcrumbTrail />
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
              <div className={`ls-header__results ${searchLoading && results.length > 0 ? "is-refreshing" : ""}`}>
                {searchLoading && results.length === 0 ? (
                  <div className="ls-header__results-empty">Searching…</div>
                ) : searchError ? (
                  <div className="ls-header__results-empty">{searchError}</div>
                ) : results.length === 0 ? (
                  <div className="ls-header__results-empty">
                    No results for <span className="mono">{query}</span>
                  </div>
                ) : (
                  <div className="ls-header__results-list">
                    {results.map((item) => (
                      <button
                        key={`${item.kind}-${item.id}`}
                        className="ls-header__result"
                        type="button"
                        onMouseDown={(e) => {
                          e.preventDefault();
                          goToResult(item);
                        }}
                      >
                        {item.image ? (
                          <img src={item.image} alt="" className="ls-header__result-img" />
                        ) : (
                          <span className="ls-header__result-img ls-header__result-img--empty">
                            {item.title.slice(0, 1)}
                          </span>
                        )}
                        <div className="ls-header__result-body">
                          <div className="ls-header__result-title">
                            {item.title}
                          </div>
                          <div className="ls-header__result-meta mono">
                            <span>{searchKindLabel(item.kind)}</span>
                            {item.subtitle ? (
                              <>
                                <span>·</span>
                                <span>{item.subtitle}</span>
                              </>
                            ) : null}
                          </div>
                        </div>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </form>
        </div>

        <div className="ls-header__actions">
          <button
            className="ls-header__icon-btn"
            aria-label="Notifications"
            type="button"
            onClick={() => setNotifOpen((v) => !v)}
          >
            <Bell size={16} strokeWidth={1.75} />
            {notifications.some(
              (item) => item.readAt === null || item.readAt === undefined,
            ) ? (
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
                    <button
                      key={notification.id}
                      type="button"
                      className="ls-header__notif"
                      onClick={() => {
                        if (
                          notification.readAt === null ||
                          notification.readAt === undefined
                        ) {
                          void markNotificationRead(notification.id);
                        }
                      }}
                    >
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
                    </button>
                  ))
                )}
              </div>
            </div>
          )}
          <button
            className="ls-header__icon-btn"
            aria-label="Settings"
            type="button"
            onClick={() => navigate("/settings?section=account")}
          >
            <Settings size={16} strokeWidth={1.75} />
          </button>
          {!signedIn ? (
            <div className="ls-header__auth-actions">
              <button
                type="button"
                className="ls-header__auth-link"
                onClick={() => openAuth("sign-in")}
              >
                <LogIn size={14} strokeWidth={1.8} />
                <span className="ls-header__auth-label">Sign in</span>
                <span className="ls-header__auth-label-short">In</span>
              </button>
              <button
                type="button"
                className="ls-header__auth-link is-primary"
                onClick={() => openAuth("sign-up")}
              >
                <UserPlus size={14} strokeWidth={1.8} />
                <span className="ls-header__auth-label">Sign up</span>
                <span className="ls-header__auth-label-short">Join</span>
              </button>
            </div>
          ) : (
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
          )}
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
              <button
                type="button"
                className="ls-header__menu-item"
                onClick={() => {
                  setUserOpen(false);
                  navigate("/settings?section=playback");
                }}
              >
                Preferences
              </button>
              <button
                type="button"
                className="ls-header__menu-item"
                onClick={signOut}
              >
                Sign out
              </button>
            </div>
          )}
        </div>
      </div>
      {authOpen ? (
        <div
          className="ls-header__auth-backdrop"
          role="presentation"
          onMouseDown={() => setAuthOpen(false)}
        >
          <form
            className="ls-header__auth-modal"
            onMouseDown={(event) => event.stopPropagation()}
            onSubmit={(event) => {
              event.preventDefault();
              void submitAuth();
            }}
            role="dialog"
            aria-modal="true"
            aria-labelledby="vanta-auth-title"
          >
            <button
              className="ls-header__auth-close"
              type="button"
              aria-label="Close"
              onClick={() => setAuthOpen(false)}
            >
              <X size={15} strokeWidth={1.8} />
            </button>
            <div className="ls-header__auth-heading">
              <div className="ls-header__auth-mark" aria-hidden="true">
                <span />
              </div>
              <div className="ls-header__popover-title mono">
                {authMode === "sign-up" ? "Create account" : "Welcome back"}
              </div>
              <h2 className="ls-header__auth-title" id="vanta-auth-title">
                {authMode === "sign-up"
                  ? "Sign up for VANTA"
                  : "Sign in to VANTA"}
              </h2>
              <p>
                {authMode === "sign-up"
                  ? "Start watching with a VANTA account."
                  : "Continue to your VANTA account."}
              </p>
            </div>
            <div className="ls-header__auth-tabs">
              <button
                type="button"
                className={authMode === "sign-in" ? "is-active" : ""}
                onClick={() => setAuthMode("sign-in")}
              >
                Sign in
              </button>
              <button
                type="button"
                className={authMode === "sign-up" ? "is-active" : ""}
                onClick={() => setAuthMode("sign-up")}
              >
                Sign up
              </button>
            </div>
            <div className="ls-header__auth-fields">
              {authMode === "sign-up" ? (
                <Input
                  className="ls-input--full"
                  icon={<User />}
                  value={authDisplayName}
                  onChange={(event) => setAuthDisplayName(event.target.value)}
                  placeholder="Display name"
                  aria-label="Display name"
                />
              ) : null}
              <Input
                className="ls-input--full"
                icon={<Mail />}
                value={authEmail}
                onChange={(event) => setAuthEmail(event.target.value)}
                placeholder="Email"
                aria-label="Email"
                type="email"
                autoComplete="email"
              />
              <Input
                className="ls-input--full"
                icon={<LockKeyhole />}
                value={authPassword}
                onChange={(event) => setAuthPassword(event.target.value)}
                placeholder="Password"
                aria-label="Password"
                type="password"
                autoComplete={
                  authMode === "sign-up" ? "new-password" : "current-password"
                }
              />
            </div>
            {authError ? (
              <div className="ls-header__auth-error">{authError}</div>
            ) : null}
            <Button
              className="ls-header__auth-submit"
              type="submit"
              disabled={authPending}
              full
            >
              {authPending
                ? "Working…"
                : authMode === "sign-up"
                  ? "Create account"
                  : "Sign in"}
            </Button>
            <div className="ls-header__auth-divider">
              <span>or</span>
            </div>
            <Button
              variant="outline"
              className="ls-header__auth-google"
              type="button"
              icon={<Globe />}
              onClick={startGoogleSignIn}
              full
            >
              Continue with Google
            </Button>
          </form>
        </div>
      ) : null}
    </header>
  );
}
