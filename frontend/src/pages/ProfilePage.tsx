import { useEffect, useMemo, useState } from "react";
import { Link as RouterLink, Navigate, useParams } from "react-router-dom";
import {
  Check,
  Copy,
  ExternalLink,
  Film,
  Link as LinkIcon,
  Pencil,
  Plus,
  Save,
  Trash2,
  Tv,
  X,
  type LucideIcon,
} from "lucide-react";
import type { IconType } from "react-icons";
import { FaFacebookF, FaImdb, FaInstagram, FaLinkedinIn, FaXTwitter } from "react-icons/fa6";
import { repository } from "@/lib/repository";
import { AlertMeButton } from "@/components/alerts/AlertMeButton";
import { PageMetadata } from "@/components/seo/PageMetadata";
import { PageTrail } from "@/components/navigation/PageTrail";
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

function creditHref(credit: PersonProfile["credits"][number]): string {
  return credit.contentKind === "series"
    ? `/series/${credit.contentSlug}`
    : `/film/${credit.contentSlug}`;
}

function creditKindLabel(kind: PersonProfile["credits"][number]["contentKind"]): string {
  return kind === "series" ? "Series" : "Film";
}

function creditDescriptor(credit: PersonProfile["credits"][number]): string {
  return [
    creditKindLabel(credit.contentKind),
    String(credit.year),
    credit.role,
    credit.character,
  ].filter(Boolean).join(" / ");
}

function CreditImage({
  credit,
  className,
}: {
  readonly credit: PersonProfile["credits"][number];
  readonly className?: string;
}) {
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [credit.poster]);

  if (!credit.poster || failed) {
    return (
      <div className={className ? `${className} ls-profile__poster-fallback` : "ls-profile__poster-fallback"} aria-hidden="true">
        <span>{credit.title}</span>
      </div>
    );
  }

  return <img className={className} src={credit.poster} alt="" loading="lazy" onError={() => setFailed(true)} />;
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
  const [expandedWorkGroup, setExpandedWorkGroup] = useState<"series" | "film" | "all" | null>(null);

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

  const mediaGroups = useMemo(() => {
    const credits = profile?.credits ?? [];
    return [
      { key: "series" as const, label: "Series", credits: credits.filter((credit) => credit.contentKind === "series") },
      { key: "film" as const, label: "Films", credits: credits.filter((credit) => credit.contentKind === "film") },
    ].filter((group) => group.credits.length > 0);
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
  const featuredCredit = profile.credits[0] ?? null;
  const seriesCount = profile.credits.filter((credit) => credit.contentKind === "series").length;
  const filmCount = profile.credits.filter((credit) => credit.contentKind === "film").length;
  const roleCount = new Set(profile.credits.map((credit) => credit.role)).size;
  const expandedCredits = expandedWorkGroup === "all"
    ? profile.credits
    : expandedWorkGroup
      ? profile.credits.filter((credit) => credit.contentKind === expandedWorkGroup)
      : [];
  const expandedTitle = expandedWorkGroup === "series"
    ? "All series work"
    : expandedWorkGroup === "film"
      ? "All film work"
      : "All work";

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
            <PageTrail
              className="ls-profile__kicker mono"
              items={[
                { label: "Dashboard", href: "/" },
                { label: profile.displayName },
              ]}
            />
            <h1 className="ls-profile__name">{profile.displayName}</h1>
            <div className="ls-profile__headline">{profile.headline || `@${profile.slug}`}</div>
            <div className="ls-profile__meta mono">
              {profile.location ? <span>{profile.location}</span> : null}
              <span>{profile.credits.length} credits</span>
              {seriesCount > 0 ? <span>{seriesCount} series</span> : null}
              {filmCount > 0 ? <span>{filmCount} films</span> : null}
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
        <>
          <section className="ls-profile__proof-grid" aria-label="Profile summary">
            <div className="ls-profile__link-card" aria-label="Public VANTA profile link">
              <div>
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
            </div>

            <div className="ls-profile__quick-stats" aria-label="Portfolio stats">
              <div><strong>{profile.credits.length}</strong><span className="mono">Credits</span></div>
              <div><strong>{seriesCount}</strong><span className="mono">Series</span></div>
              <div><strong>{filmCount}</strong><span className="mono">Films</span></div>
              <div><strong>{roleCount}</strong><span className="mono">Roles</span></div>
            </div>

            <div className="ls-profile__external-links">
              <div className="ls-list__label mono">Connected</div>
              <div className="ls-profile__link-pills">
                {links.length === 0 ? <span className="ls-profile__empty-pill">No public links yet</span> : null}
                {links.map(({ label, href, Icon }) => (
                  <a className="ls-profile__link-pill" key={`${label}-${href}`} href={href} target="_blank" rel="noreferrer">
                    <Icon size={14} />
                    <span>{label}</span>
                    <ExternalLink size={12} />
                  </a>
                ))}
              </div>
            </div>
          </section>

          <section className="ls-profile__showcase" aria-label="Creator media">
            <div className="ls-profile__section-head">
              <div className="ls-list__label mono">Creator media</div>
              <h2>Work at a glance</h2>
            </div>
            {featuredCredit ? (
              <RouterLink className="ls-profile__featured-work" to={creditHref(featuredCredit)}>
                <CreditImage credit={featuredCredit} className="ls-profile__featured-image" />
                <div className="ls-profile__featured-scrim" />
                <div className="ls-profile__featured-copy">
                  <span className="mono">{creditDescriptor(featuredCredit)}</span>
                  <strong>{featuredCredit.title}</strong>
                  <em>{featuredCredit.character ? `As ${featuredCredit.character}` : "View project"}</em>
                </div>
              </RouterLink>
            ) : (
              <div className="ls-profile__state">No published VANTA work yet.</div>
            )}
            {mediaGroups.map((group) => (
              <div className="ls-profile__media-rail" key={group.key}>
                <div className="ls-profile__rail-head">
                  <span className="mono">{group.label}</span>
                  <button
                    type="button"
                    onClick={() => setExpandedWorkGroup(group.key)}
                  >
                    {group.credits.length > 6 ? `See all ${group.credits.length}` : `Open ${group.credits.length}`}
                  </button>
                </div>
                <div className="ls-profile__media-strip">
                  {group.credits.slice(0, 6).map((credit) => (
                    <RouterLink className="ls-profile__media-card" key={`${credit.contentKind}-${credit.contentId}-${credit.role}`} to={creditHref(credit)}>
                      <CreditImage credit={credit} />
                      <span className="ls-profile__media-kind">
                        {credit.contentKind === "series" ? <Tv size={14} /> : <Film size={14} />}
                        {creditKindLabel(credit.contentKind)}
                      </span>
                      <strong>{credit.title}</strong>
                      <em>{credit.year} / {credit.role}</em>
                    </RouterLink>
                  ))}
                </div>
              </div>
            ))}
            {profile.credits.length > 10 ? (
              <Button variant="outline" onClick={() => setExpandedWorkGroup("all")}>
                See full portfolio
              </Button>
            ) : null}
          </section>

          <section className="ls-profile__about">
            <div>
              <div className="ls-list__label mono">About</div>
              <p>{profile.about || `${profile.displayName} has not added a bio yet.`}</p>
            </div>
            <div className="ls-profile__side">
              <div className="ls-list__label mono">Known for</div>
              {profile.knownFor.length === 0 ? (
                <div className="ls-profile__side-row">
                  <span>Focus</span>
                  <span>Not listed yet</span>
                </div>
              ) : profile.knownFor.map((item) => (
                <div className="ls-profile__side-row" key={item}>
                  <span>Focus</span>
                  <span>{item}</span>
                </div>
              ))}
              <div className="ls-profile__side-row">
                <span>Profile</span>
                <span>{profileUrlPath}</span>
              </div>
            </div>
          </section>
        </>
      )}

      <section className="ls-profile__filmography">
        <div className="ls-profile__section-head">
          <div className="ls-list__label mono">Credits</div>
          <h2>Full credit list</h2>
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
                    to={creditHref(credit)}
                  >
                    <CreditImage credit={credit} />
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

      {expandedWorkGroup ? (
        <div className="ls-profile__modal-backdrop" role="presentation" onMouseDown={() => setExpandedWorkGroup(null)}>
          <section
            className="ls-profile__work-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="profile-work-title"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="ls-profile__work-modal-head">
              <div>
                <div className="ls-list__label mono">Portfolio</div>
                <h2 id="profile-work-title">{expandedTitle}</h2>
                <p>{expandedCredits.length} project{expandedCredits.length === 1 ? "" : "s"} by {profile.displayName}</p>
              </div>
              <button type="button" aria-label="Close portfolio" onClick={() => setExpandedWorkGroup(null)}>
                <X size={16} strokeWidth={1.8} />
              </button>
            </div>
            <div className="ls-profile__work-grid">
              {expandedCredits.map((credit) => (
                <RouterLink
                  className="ls-profile__work-card"
                  key={`expanded-${credit.contentKind}-${credit.contentId}-${credit.role}`}
                  to={creditHref(credit)}
                  onClick={() => setExpandedWorkGroup(null)}
                >
                  <CreditImage credit={credit} />
                  <span className="mono">{creditDescriptor(credit)}</span>
                  <strong>{credit.title}</strong>
                  {credit.character ? <em>{credit.character}</em> : null}
                </RouterLink>
              ))}
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}
