import { useEffect, useMemo, useState } from "react";
import { Link as RouterLink, Navigate, useParams } from "react-router-dom";
import {
  Check,
  Copy,
  ExternalLink,
  Link as LinkIcon,
  Pencil,
  Plus,
  Save,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import type { IconType } from "react-icons";
import { FaFacebookF, FaImdb, FaInstagram, FaLinkedinIn, FaXTwitter } from "react-icons/fa6";
import { repository } from "@/lib/repository";
import { AlertMeButton } from "@/components/alerts/AlertMeButton";
import { PageMetadata } from "@/components/seo/PageMetadata";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { useAppStore } from "@/lib/store";
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
  linkedinUrl: string;
  facebookUrl: string;
  publicLinks: Array<{
    platform: string;
    label: string;
    url: string;
  }>;
};

const standardLinkFields = [
  { key: "websiteUrl", label: "Website", platform: "website", Icon: LinkIcon },
  { key: "instagramUrl", label: "Instagram", platform: "instagram", Icon: FaInstagram },
  { key: "xUrl", label: "X", platform: "x", Icon: FaXTwitter },
  { key: "imdbUrl", label: "IMDb", platform: "imdb", Icon: FaImdb },
  { key: "linkedinUrl", label: "LinkedIn", platform: "linkedin", Icon: FaLinkedinIn },
  { key: "facebookUrl", label: "Facebook", platform: "facebook", Icon: FaFacebookF },
] as const satisfies ReadonlyArray<{
  key: keyof Pick<
    PersonForm,
    "websiteUrl" | "instagramUrl" | "xUrl" | "imdbUrl" | "linkedinUrl" | "facebookUrl"
  >;
  label: string;
  platform: string;
  Icon: LucideIcon | IconType;
}>;

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
    linkedinUrl: profile.linkedinUrl ?? "",
    facebookUrl: profile.facebookUrl ?? "",
    publicLinks: profile.publicLinks.map((link) => ({
      platform: link.platform || "custom",
      label: link.label,
      url: link.url,
    })),
  };
}

function nullableUrl(value: string): string | null {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function profileInitials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("") || "V";
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
    websiteUrl: nullableUrl(form.websiteUrl),
    instagramUrl: nullableUrl(form.instagramUrl),
    xUrl: nullableUrl(form.xUrl),
    imdbUrl: nullableUrl(form.imdbUrl),
    linkedinUrl: nullableUrl(form.linkedinUrl),
    facebookUrl: nullableUrl(form.facebookUrl),
    publicLinks: form.publicLinks
      .map((link) => ({
        platform: link.platform.trim() || "custom",
        label: link.label.trim(),
        url: link.url.trim(),
      }))
      .filter((link) => link.label && link.url),
  };
}

export function ProfilePage() {
  const { profileHandle, slug } = useParams<{ profileHandle?: string; slug?: string }>();
  const currentUser = useAppStore((state) => state.user);
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

  const updateCustomLink = (
    index: number,
    field: "platform" | "label" | "url",
    value: string,
  ) => {
    setForm((current) => {
      if (!current) return current;
      return {
        ...current,
        publicLinks: current.publicLinks.map((link, linkIndex) =>
          linkIndex === index ? { ...link, [field]: value } : link,
        ),
      };
    });
  };

  const addCustomLink = () => {
    setForm((current) => current
      ? {
          ...current,
          publicLinks: [...current.publicLinks, { platform: "custom", label: "", url: "" }],
        }
      : current);
  };

  const removeCustomLink = (index: number) => {
    setForm((current) => current
      ? {
          ...current,
          publicLinks: current.publicLinks.filter((_, linkIndex) => linkIndex !== index),
        }
      : current);
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

  const links = [
    ...standardLinkFields
      .flatMap(({ key, label, platform, Icon }) => {
        const href = profile[key];
        return href ? [{ label, platform, Icon, href }] : [];
      }),
    ...profile.publicLinks.map((link) => ({
      label: link.label,
      platform: link.platform || "custom",
      Icon: LinkIcon,
      href: link.url,
    })),
  ];
  const profileUrlPath = profile.profileUrlPath || `/@${profile.slug}`;
  const profileUrl = new URL(profileUrlPath, window.location.origin).href;
  const viewingOwnPublicProfile = profile.userId !== null && profile.userId !== undefined && profile.userId === currentUser.id;
  const heroStyle = profile.heroImage || profile.avatar
    ? { backgroundImage: `url(${profile.heroImage || profile.avatar})` }
    : undefined;

  return (
    <div className="ls-profile">
      <PageMetadata
        title={`${profile.displayName} - VANTA creator profile`}
        description={`${profile.headline || profile.about || `${profile.displayName} on VANTA`}. View ${profile.displayName}'s creator profile, public links, and long-form credits on VANTA.`}
        path={profileUrlPath}
        image={profile.heroImage || profile.avatar}
        type="profile"
        structuredData={{
          "@context": "https://schema.org",
          "@type": "Person",
          name: profile.displayName,
          alternateName: `@${profile.slug}`,
          description: profile.about || profile.headline,
          image: profile.avatar || profile.heroImage,
          url: `https://streamvanta.tv${profileUrlPath}`,
          sameAs: links.map((link) => link.href),
          knowsAbout: profile.knownFor,
          workExample: profile.credits.map((credit) => ({
            "@type": credit.contentKind === "series" ? "TVSeries" : "Movie",
            name: credit.title,
            datePublished: String(credit.year),
            url: `https://streamvanta.tv/${credit.contentKind === "series" ? "series" : "film"}/${credit.contentSlug}`,
            contributor: {
              "@type": "Person",
              name: profile.displayName,
              roleName: credit.role,
            },
          })),
        }}
      />
      <section
        className="ls-profile__hero"
        style={heroStyle}
      >
        <div className="ls-profile__hero-scrim" />
        <div className="ls-profile__identity">
          {profile.avatar ? (
            <img className="ls-profile__avatar" src={profile.avatar} alt="" />
          ) : (
            <div className="ls-profile__avatar ls-profile__avatar--fallback">
              {profileInitials(profile.displayName)}
            </div>
          )}
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
          <div className="ls-profile__actions">
          {isOwnProfile ? (
            <>
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
            </>
          ) : !viewingOwnPublicProfile ? (
            <AlertMeButton
              targetKind="profile"
              targetId={profile.id}
              targetSlug={profile.slug}
              targetTitle={profile.displayName}
              alertTypes={["creator_update", "new_episode", "series_drop"]}
            />
          ) : null}
          </div>
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
          <div className="ls-profile__links-editor">
            <div className="ls-list__label mono">Public links</div>
            <div className="ls-profile__standard-links">
              {standardLinkFields.map(({ key, label, Icon }) => (
                <label className="ls-profile__link-field" key={key}>
                  <span><Icon size={14} />{label}</span>
                  <Input
                    value={form[key]}
                    onChange={(event) => updateField(key, event.target.value)}
                    placeholder={`${label} URL`}
                  />
                </label>
              ))}
            </div>
            <div className="ls-profile__custom-links">
              <div className="ls-profile__custom-head">
                <span className="mono">Custom</span>
                <Button variant="outline" icon={<Plus />} onClick={addCustomLink}>Add link</Button>
              </div>
              {form.publicLinks.map((link, index) => (
                <div className="ls-profile__custom-link" key={index}>
                  <Input
                    value={link.label}
                    onChange={(event) => updateCustomLink(index, "label", event.target.value)}
                    placeholder="Label"
                  />
                  <Input
                    value={link.url}
                    onChange={(event) => updateCustomLink(index, "url", event.target.value)}
                    placeholder="https://..."
                  />
                  <button
                    className="ls-profile__remove-link"
                    type="button"
                    aria-label={`Remove ${link.label || "custom link"}`}
                    onClick={() => removeCustomLink(index)}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              ))}
            </div>
          </div>
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
            <div className="ls-list__label mono">Connected</div>
            {links.length === 0 ? (
              <div className="ls-profile__side-row">
                <span>Public links</span>
                <span>None yet</span>
              </div>
            ) : null}
            {links.map(({ label, href, Icon }) => (
              <a className="ls-profile__side-row" key={`${label}-${href}`} href={href} target="_blank" rel="noreferrer">
                <span><Icon size={13} />{label}</span>
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
                  <RouterLink
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
                  </RouterLink>
                ))}
              </div>
            </div>
          ))
        )}
      </section>
    </div>
  );
}
