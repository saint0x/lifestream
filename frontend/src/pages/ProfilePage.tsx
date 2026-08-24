import { useEffect, useMemo, useState } from "react";
import { Link, Navigate, useParams } from "react-router-dom";
import { Check, Copy, ExternalLink, Pencil, Save } from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import type { PersonProfile, UpdatePersonProfileRequest } from "@/types";
import "./ProfilePage.css";

type PersonForm = {
  slug: string;
  displayName: string;
  avatar: string;
  heroImage: string;
  headline: string;
  location: string;
  about: string;
  knownFor: string;
  websiteUrl: string;
  instagramUrl: string;
  xUrl: string;
  imdbUrl: string;
};

function formFromProfile(profile: PersonProfile): PersonForm {
  return {
    slug: profile.slug,
    displayName: profile.displayName,
    avatar: profile.avatar,
    heroImage: profile.heroImage,
    headline: profile.headline,
    location: profile.location,
    about: profile.about,
    knownFor: profile.knownFor.join(", "),
    websiteUrl: profile.websiteUrl ?? "",
    instagramUrl: profile.instagramUrl ?? "",
    xUrl: profile.xUrl ?? "",
    imdbUrl: profile.imdbUrl ?? "",
  };
}

function payloadFromForm(form: PersonForm): UpdatePersonProfileRequest {
  return {
    slug: form.slug,
    displayName: form.displayName,
    avatar: form.avatar,
    heroImage: form.heroImage,
    headline: form.headline,
    location: form.location,
    about: form.about,
    knownFor: form.knownFor
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean),
    websiteUrl: form.websiteUrl || null,
    instagramUrl: form.instagramUrl || null,
    xUrl: form.xUrl || null,
    imdbUrl: form.imdbUrl || null,
  };
}

export function ProfilePage() {
  const { profileHandle, slug } = useParams<{ profileHandle?: string; slug?: string }>();
  const user = useAppStore((s) => s.user);
  const publicSlug = slug ?? (profileHandle?.startsWith("@") ? profileHandle.slice(1) : undefined);
  const isOwnProfile = publicSlug === undefined && profileHandle === undefined;
  const [profile, setProfile] = useState<PersonProfile | null>(null);
  const [form, setForm] = useState<PersonForm | null>(null);
  const [editing, setEditing] = useState(false);
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOwnProfile && !publicSlug) {
      setProfile(null);
      setForm(null);
      setLoading(false);
      return;
    }

    const controller = new AbortController();
    setLoading(true);
    setError(null);
    setStatus(null);
    setCopyStatus(null);
    setEditing(false);

    const request = isOwnProfile
      ? repository.fetchMyPersonProfile(controller.signal)
      : repository.fetchPersonProfile(publicSlug!, controller.signal);

    void request
      .then((nextProfile) => {
        setProfile(nextProfile);
        setForm(formFromProfile(nextProfile));
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setProfile(null);
          setForm(null);
          setError(err instanceof Error ? err.message : "Unable to load this profile.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [isOwnProfile, publicSlug]);

  const groupedCredits = useMemo(() => {
    const groups = new Map<string, PersonProfile["credits"]>();
    for (const credit of profile?.credits ?? []) {
      const key = credit.role;
      groups.set(key, [...(groups.get(key) ?? []), credit]);
    }
    return Array.from(groups.entries());
  }, [profile]);

  const updateField = (field: keyof PersonForm, value: string) => {
    setForm((current) => (current ? { ...current, [field]: value } : current));
  };

  const saveProfile = async () => {
    if (!form) return;
    setStatus("Saving profile...");
    setError(null);
    try {
      const nextProfile = await repository.updateMyPersonProfile(payloadFromForm(form));
      setProfile(nextProfile);
      setForm(formFromProfile(nextProfile));
      setEditing(false);
      setCopyStatus(null);
      setStatus("Profile updated.");
    } catch (err) {
      setStatus(null);
      setError(err instanceof Error ? err.message : "Unable to save profile.");
    }
  };

  const copyProfileUrl = async () => {
    if (!profile) return;
    const path = profile.profileUrlPath || `/@${profile.slug}`;
    const url = new URL(path, window.location.origin).href;
    try {
      await navigator.clipboard.writeText(url);
      setStatus(null);
      setError(null);
      setCopyStatus("Profile link copied.");
    } catch {
      setCopyStatus(null);
      setError("Unable to copy profile link.");
    }
  };

  if (!isOwnProfile && !publicSlug) return <Navigate to="/" replace />;

  if (loading || !profile || !form) {
    return (
      <div className="ls-profile">
        <div className="ls-profile__state">
          {loading ? "Loading profile..." : error ?? "Profile is unavailable."}
        </div>
      </div>
    );
  }

  const links: Array<[string, string]> = [];
  if (profile.websiteUrl) links.push(["Website", profile.websiteUrl]);
  if (profile.instagramUrl) links.push(["Instagram", profile.instagramUrl]);
  if (profile.xUrl) links.push(["X", profile.xUrl]);
  if (profile.imdbUrl) links.push(["IMDb", profile.imdbUrl]);
  const profileUrlPath = profile.profileUrlPath || `/@${profile.slug}`;
  const profileUrl = new URL(profileUrlPath, window.location.origin).href;

  return (
    <div className="ls-profile">
      <section
        className="ls-profile__hero"
        style={{ backgroundImage: `url(${profile.heroImage || profile.avatar})` }}
      >
        <div className="ls-profile__hero-scrim" />
        <div className="ls-profile__identity">
          <img className="ls-profile__avatar" src={profile.avatar} alt="" />
          <div className="ls-profile__intro">
            <div className="ls-profile__kicker mono">
              {isOwnProfile ? "your public profile" : "person profile"}
            </div>
            <h1 className="ls-profile__name">{profile.displayName}</h1>
            <div className="ls-profile__headline">{profile.headline || `@${profile.slug}`}</div>
            <div className="ls-profile__meta mono">
              {profile.location ? <span>{profile.location}</span> : null}
              <span>{profile.credits.length} credits</span>
              {profile.knownFor.map((item) => (
                <span key={item}>{item}</span>
              ))}
            </div>
          </div>
          {isOwnProfile ? (
            <div className="ls-profile__actions">
              <Button
                variant={editing ? "primary" : "outline"}
                icon={editing ? <Save /> : <Pencil />}
                onClick={() => {
                  if (editing) void saveProfile();
                  else setEditing(true);
                }}
              >
                {editing ? "Save" : "Edit"}
              </Button>
              {editing ? (
                <Button
                  variant="ghost"
                  onClick={() => {
                    setForm(formFromProfile(profile));
                    setEditing(false);
                    setError(null);
                    setStatus(null);
                  }}
                >
                  Cancel
                </Button>
              ) : null}
            </div>
          ) : null}
        </div>
      </section>

      {(status || error || copyStatus) ? (
        <div className={error ? "ls-profile__message is-error" : "ls-profile__message"}>
          {error ?? status ?? copyStatus}
        </div>
      ) : null}

      <section className="ls-profile__link" aria-label="Public VANTA profile link">
        <div className="ls-profile__link-copy">
          <div className="ls-list__label mono">VANTA link</div>
          <div className="ls-profile__link-url">{profileUrl}</div>
        </div>
        <Button
          variant={copyStatus ? "primary" : "outline"}
          icon={copyStatus ? <Check /> : <Copy />}
          onClick={() => void copyProfileUrl()}
        >
          {copyStatus ? "Copied" : "Copy"}
        </Button>
      </section>

      {editing ? (
        <section className="ls-profile__editor" aria-label="Edit public profile">
          <Input value={form.displayName} onChange={(event) => updateField("displayName", event.target.value)} placeholder="Display name" />
          <Input value={form.slug} onChange={(event) => updateField("slug", event.target.value)} placeholder="VANTA link slug" />
          <Input value={form.headline} onChange={(event) => updateField("headline", event.target.value)} placeholder="Headline" />
          <Input value={form.location} onChange={(event) => updateField("location", event.target.value)} placeholder="Location" />
          <Input value={form.avatar} onChange={(event) => updateField("avatar", event.target.value)} placeholder="Avatar URL" />
          <Input value={form.heroImage} onChange={(event) => updateField("heroImage", event.target.value)} placeholder="Hero image URL" />
          <Input value={form.knownFor} onChange={(event) => updateField("knownFor", event.target.value)} placeholder="Known for, comma separated" />
          <Input value={form.websiteUrl} onChange={(event) => updateField("websiteUrl", event.target.value)} placeholder="Website URL" />
          <Input value={form.instagramUrl} onChange={(event) => updateField("instagramUrl", event.target.value)} placeholder="Instagram URL" />
          <Input value={form.xUrl} onChange={(event) => updateField("xUrl", event.target.value)} placeholder="X URL" />
          <Input value={form.imdbUrl} onChange={(event) => updateField("imdbUrl", event.target.value)} placeholder="IMDb URL" />
          <label className="ls-profile__textarea">
            <textarea
              value={form.about}
              onChange={(event) => updateField("about", event.target.value)}
              placeholder="About"
              rows={7}
            />
          </label>
        </section>
      ) : (
        <section className="ls-profile__about">
          <div>
            <div className="ls-list__label mono">About</div>
            <p>{profile.about || `${profile.displayName} has not added a bio yet.`}</p>
          </div>
          <div className="ls-profile__side">
            <div className="ls-list__label mono">Profile</div>
            <div className="ls-profile__side-row">
              <span>Link</span>
              <span>{profileUrlPath}</span>
            </div>
            {profile.userId === user.id ? (
              <div className="ls-profile__side-row">
                <span>Account</span>
                <span>Linked</span>
              </div>
            ) : null}
            {links.map(([label, href]) => (
              <a className="ls-profile__side-row" key={label} href={href} target="_blank" rel="noreferrer">
                <span>{label}</span>
                <ExternalLink size={13} />
              </a>
            ))}
          </div>
        </section>
      )}

      <section className="ls-profile__filmography">
        <div className="ls-profile__section-head">
          <div className="ls-list__label mono">Filmography</div>
          <h2>Credits</h2>
        </div>
        {groupedCredits.length === 0 ? (
          <div className="ls-profile__state">No credits yet.</div>
        ) : (
          groupedCredits.map(([role, credits]) => (
            <div className="ls-profile__credit-group" key={role}>
              <div className="ls-profile__role mono">{role}</div>
              <div className="ls-profile__credits">
                {credits.map((credit) => (
                  <Link
                    className="ls-profile__credit"
                    key={`${credit.contentKind}-${credit.contentId}-${credit.role}`}
                    to={credit.contentKind === "series" ? `/series/${credit.contentSlug}` : `/film/${credit.contentSlug}`}
                  >
                    <img src={credit.poster} alt="" />
                    <div>
                      <div className="ls-profile__credit-title">{credit.title}</div>
                      <div className="ls-profile__credit-meta mono">
                        {credit.year}
                        {credit.character ? ` / ${credit.character}` : ""}
                      </div>
                    </div>
                  </Link>
                ))}
              </div>
            </div>
          ))
        )}
      </section>
    </div>
  );
}
