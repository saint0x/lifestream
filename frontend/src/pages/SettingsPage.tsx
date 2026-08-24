import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  User as UserIcon,
  Monitor,
  Bell,
  Shield,
  Lock,
  Download,
  CreditCard,
  Globe,
  LogOut,
} from "lucide-react";
import { useAppStore } from "@/lib/store";
import { Avatar } from "@/components/ui/Avatar";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { PageTrail } from "@/components/navigation/PageTrail";
import type { UserSettingsBundle } from "@/types";
import "./SettingsPage.css";

interface SectionDef {
  readonly key: string;
  readonly label: string;
  readonly Icon: typeof UserIcon;
}

const sections: ReadonlyArray<SectionDef> = [
  { key: "account", label: "Account", Icon: UserIcon },
  { key: "playback", label: "Playback", Icon: Monitor },
  { key: "notifications", label: "Notifications", Icon: Bell },
  { key: "privacy", label: "Privacy", Icon: Lock },
  { key: "parental", label: "Parental", Icon: Shield },
  { key: "downloads", label: "Downloads", Icon: Download },
  { key: "plan", label: "Plan & Billing", Icon: CreditCard },
  { key: "language", label: "Language & Region", Icon: Globe },
];

function settingsSectionFromParam(value: string | null): string {
  return sections.some((item) => item.key === value) ? value ?? "account" : "account";
}

export function SettingsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const user = useAppStore((s) => s.user);
  const profile = useAppStore((s) => s.profile);
  const settings = useAppStore((s) => s.settings);
  const plan = useAppStore((s) => s.plan);
  const sessions = useAppStore((s) => s.sessions);
  const signOut = useAppStore((s) => s.signOut);
  const updateProfile = useAppStore((s) => s.updateProfile);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const revokeSession = useAppStore((s) => s.revokeSession);
  const [section, setSection] = useState<string>(
    settingsSectionFromParam(searchParams.get("section")),
  );
  const [draft, setDraft] = useState<UserSettingsBundle>(settings);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [profileOpen, setProfileOpen] = useState(false);
  const [displayName, setDisplayName] = useState(user.displayName);
  const [email, setEmail] = useState(profile.email);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  useEffect(() => {
    setSection(settingsSectionFromParam(searchParams.get("section")));
    setProfileOpen(searchParams.get("edit") === "profile");
  }, [searchParams]);

  const saveSettings = async () => {
    setSaving(true);
    setStatus(null);
    try {
      await updateSettings(draft);
      setStatus("Settings saved.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Unable to save settings.");
    } finally {
      setSaving(false);
    }
  };

  const saveProfile = async () => {
    setSaving(true);
    setStatus(null);
    try {
      await updateProfile({
        displayName: displayName.trim(),
        email: email.trim(),
      });
      setProfileOpen(false);
      setStatus("Profile saved.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Unable to save profile.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="ls-settings">
      <header className="ls-settings__head">
        <PageTrail
          className="ls-settings__kicker mono"
          items={[
            { label: "Dashboard", href: "/" },
            { label: "Settings" },
          ]}
        />
        <h1 className="ls-settings__title">Settings</h1>
        <p className="ls-settings__sub">
          Preferences, playback defaults, privacy, billing — apply to every device you
          sign in from.
        </p>
      </header>

      <div className="ls-settings__layout">
        <nav className="ls-settings__nav">
          {sections.map(({ key, label, Icon }) => (
            <button
              key={key}
              type="button"
              className={`ls-settings__nav-item ${section === key ? "is-active" : ""}`}
              onClick={() => setSearchParams({ section: key })}
            >
              <Icon size={14} strokeWidth={1.75} />
              <span>{label}</span>
            </button>
          ))}
          <div className="ls-settings__nav-sep" />
          <button
            type="button"
            className="ls-settings__nav-item ls-settings__nav-item--danger"
            onClick={signOut}
          >
            <LogOut size={14} strokeWidth={1.75} />
            <span>Sign out</span>
          </button>
        </nav>

        <div className="ls-settings__panel">
          {section === "account" && (
            <div className="ls-settings__section">
              <div className="ls-settings__sec-head">
                <div>
                  <h2 className="ls-settings__sec-title">Account</h2>
                  <p className="ls-settings__sec-sub">Identity, email and authentication.</p>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setProfileOpen((open) => !open)}
                >
                  Edit profile
                </Button>
              </div>

              {status ? <div className="ls-settings__notice">{status}</div> : null}

              {profileOpen ? (
                <div className="ls-settings__edit">
                  <label>
                    <span className="mono">Display name</span>
                    <input value={displayName} onChange={(event) => setDisplayName(event.target.value)} />
                  </label>
                  <label>
                    <span className="mono">Email</span>
                    <input value={email} onChange={(event) => setEmail(event.target.value)} />
                  </label>
                  <div className="ls-settings__plan-actions">
                    <Button variant="outline" onClick={() => setProfileOpen(false)} disabled={saving}>Cancel</Button>
                    <Button variant="primary" onClick={() => void saveProfile()} disabled={saving}>
                      {saving ? "Saving…" : "Save profile"}
                    </Button>
                  </div>
                </div>
              ) : null}

              <div className="ls-settings__account">
                <Avatar src={user.avatar} alt={user.displayName} size={64} />
                <div className="ls-settings__account-body">
                  <div className="ls-settings__account-name">
                    {user.displayName}
                    <Badge tone="premium">{user.tier.toUpperCase()}</Badge>
                  </div>
                  <div className="ls-settings__account-handle mono">
                    @{user.handle} · joined {user.joinedAt}
                  </div>
                </div>
              </div>

              <Field label="Display name" value={user.displayName} />
              <Field label="Handle" value={`@${user.handle}`} />
              <Field
                label="Email"
                value={profile.email}
                badge={profile.emailVerified ? "verified" : "unverified"}
              />
              <Field label="Password" value="•••••••••••" />
              <Field
                label="Two-factor"
                value="Authenticator app"
                badge="enabled"
              />
              <Field
                label="Connected accounts"
                value={
                  profile.connectedAccounts.length > 0
                    ? profile.connectedAccounts.map((account) => account.displayName).join(", ")
                    : "None connected"
                }
              />
              <div className="ls-settings__sessions">
                <div className="ls-settings__field-label mono">Sessions</div>
                {sessions.map((session) => (
                  <div key={session.id} className="ls-settings__session-row">
                    <div>
                      <div className="ls-settings__session-title">
                        {session.label}
                        {session.isCurrent ? <Badge tone="new">CURRENT</Badge> : null}
                      </div>
                      <div className="ls-settings__session-meta mono">
                        {session.scopes.join(" / ")} · {session.lastUsedAt ?? session.createdAt}
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={session.isCurrent || saving}
                      onClick={() => {
                        setSaving(true);
                        setStatus(null);
                        void revokeSession(session.id)
                          .then(() => setStatus("Session revoked."))
                          .catch((error) => {
                            setStatus(error instanceof Error ? error.message : "Unable to revoke session.");
                          })
                          .finally(() => setSaving(false));
                      }}
                    >
                      Revoke
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {section === "playback" && (
            <div className="ls-settings__section">
              <div className="ls-settings__sec-head">
                <div>
                  <h2 className="ls-settings__sec-title">Playback</h2>
                  <p className="ls-settings__sec-sub">
                    Defaults applied to every new playback session.
                  </p>
                </div>
              </div>
              <Select label="Default quality" value={draft.playback.defaultQuality} options={[draft.playback.defaultQuality, "1080p", "720p", "Data saver"]} onChange={(value) => setDraft({ ...draft, playback: { ...draft.playback, defaultQuality: value } })} />
              <Select label="Audio" value={draft.playback.audioLanguage} options={[draft.playback.audioLanguage, "Original language"]} onChange={(value) => setDraft({ ...draft, playback: { ...draft.playback, audioLanguage: value } })} />
              <Select label="Subtitles" value={draft.playback.subtitleStyle} options={[draft.playback.subtitleStyle, "Off"]} onChange={(value) => setDraft({ ...draft, playback: { ...draft.playback, subtitleStyle: value } })} />
              <Switch label="Autoplay next episode" on={draft.playback.autoplayNextEpisode} onChange={(value) => setDraft({ ...draft, playback: { ...draft.playback, autoplayNextEpisode: value } })} />
              <Switch label="Autoplay trailers on hover" on={draft.playback.autoplayTrailers} onChange={(value) => setDraft({ ...draft, playback: { ...draft.playback, autoplayTrailers: value } })} />
              <Switch label="Reduced motion" on={draft.playback.reducedMotion} onChange={(value) => setDraft({ ...draft, playback: { ...draft.playback, reducedMotion: value } })} />
              <Switch label="Prefer dubbed over subtitles" on={draft.playback.preferDubbed} onChange={(value) => setDraft({ ...draft, playback: { ...draft.playback, preferDubbed: value } })} />
              <Select label="Playback speed default" value={draft.playback.playbackSpeed} options={[draft.playback.playbackSpeed, "1× (normal)", "1.25×", "1.5×", "1.75×", "2×"]} onChange={(value) => setDraft({ ...draft, playback: { ...draft.playback, playbackSpeed: value } })} />
              <SettingsSaveBar saving={saving} status={status} onReset={() => setDraft(settings)} onSave={() => void saveSettings()} />
            </div>
          )}

          {section === "notifications" && (
            <div className="ls-settings__section">
              <div className="ls-settings__sec-head">
                <div>
                  <h2 className="ls-settings__sec-title">Notifications</h2>
                  <p className="ls-settings__sec-sub">
                    Choose what pings, where. Email is off by default.
                  </p>
                </div>
              </div>
              {Object.entries(draft.notifications).map(([key, channel]) => (
                <ChannelRow
                  key={key}
                  label={channel.label}
                  push={channel.push}
                  email={channel.email}
                  lock={channel.lock}
                  onChange={(next) => setDraft({
                    ...draft,
                    notifications: {
                      ...draft.notifications,
                      [key]: { ...channel, ...next },
                    },
                  })}
                />
              ))}
              <SettingsSaveBar saving={saving} status={status} onReset={() => setDraft(settings)} onSave={() => void saveSettings()} />
            </div>
          )}

          {section === "privacy" && (
            <div className="ls-settings__section">
              <div className="ls-settings__sec-head">
                <div>
                  <h2 className="ls-settings__sec-title">Privacy</h2>
                  <p className="ls-settings__sec-sub">
                    Control what the platform learns about your habits.
                  </p>
                </div>
              </div>
              <Switch label="Show my activity in 'what your friends are watching'" on={draft.privacy.showFriendActivity} onChange={(value) => setDraft({ ...draft, privacy: { ...draft.privacy, showFriendActivity: value } })} />
              <Switch label="Use my viewing history to improve recommendations" on={draft.privacy.improveRecommendations} onChange={(value) => setDraft({ ...draft, privacy: { ...draft.privacy, improveRecommendations: value } })} />
              <Switch label="Allow personalized ads on live streams" on={draft.privacy.personalizedAds} onChange={(value) => setDraft({ ...draft, privacy: { ...draft.privacy, personalizedAds: value } })} />
              <Switch label="Participate in anonymous A/B tests" on={draft.privacy.abTests} onChange={(value) => setDraft({ ...draft, privacy: { ...draft.privacy, abTests: value } })} />
              <Field label="Download my data" value={`ZIP export, ~${settings.privacy.dataExportSizeMb} MB`} />
              <Field label="Delete account" value={`Permanent · ${settings.privacy.deleteCooldownDays} day cooldown`} />
              <SettingsSaveBar saving={saving} status={status} onReset={() => setDraft(settings)} onSave={() => void saveSettings()} />
            </div>
          )}

          {section === "parental" && (
            <div className="ls-settings__section">
              <div className="ls-settings__sec-head">
                <div>
                  <h2 className="ls-settings__sec-title">Parental controls</h2>
                  <p className="ls-settings__sec-sub">
                    Restrict profiles and lock maturity ratings with a 4-digit PIN.
                  </p>
                </div>
                <Badge tone="new">{settings.parental.pinSet ? "PIN SET" : "PIN OFF"}</Badge>
              </div>
              <Select label="Maximum maturity rating" value={draft.parental.maxRating} options={[draft.parental.maxRating, "G", "PG", "PG-13", "TV-14", "TV-MA / R"]} onChange={(value) => setDraft({ ...draft, parental: { ...draft.parental, maxRating: value } })} />
              <Switch label="Require PIN for mature content" on={draft.parental.requirePinForMature} onChange={(value) => setDraft({ ...draft, parental: { ...draft.parental, requirePinForMature: value } })} />
              <Switch label="Hide live chat for kids profiles" on={draft.parental.hideLiveChatForKids} onChange={(value) => setDraft({ ...draft, parental: { ...draft.parental, hideLiveChatForKids: value } })} />
              <Switch label="Block live streams tagged 'mature'" on={draft.parental.blockMatureLiveStreams} onChange={(value) => setDraft({ ...draft, parental: { ...draft.parental, blockMatureLiveStreams: value } })} />
              <Field label="PIN" value="•••• " />
              <SettingsSaveBar saving={saving} status={status} onReset={() => setDraft(settings)} onSave={() => void saveSettings()} />
            </div>
          )}

          {section === "downloads" && (
            <div className="ls-settings__section">
              <div className="ls-settings__sec-head">
                <div>
                  <h2 className="ls-settings__sec-title">Downloads</h2>
                  <p className="ls-settings__sec-sub">
                    Offline playback for series, films, and chaptered VODs.
                  </p>
                </div>
              </div>
              <Select label="Video quality" value={draft.downloads.videoQuality} options={[draft.downloads.videoQuality, "Standard (720p)", "High (1080p)", "Ultra (4K)"]} onChange={(value) => setDraft({ ...draft, downloads: { ...draft.downloads, videoQuality: value } })} />
              <Switch label="Only download over Wi-Fi" on={draft.downloads.wifiOnly} onChange={(value) => setDraft({ ...draft, downloads: { ...draft.downloads, wifiOnly: value } })} />
              <Switch label="Smart downloads — keep next 3 episodes ready" on={draft.downloads.smartDownloads} onChange={(value) => setDraft({ ...draft, downloads: { ...draft.downloads, smartDownloads: value } })} />
              <Field label="Storage used" value={`${settings.downloads.storageUsedGb} GB of ${settings.downloads.storageLimitGb} GB`} />
              <Field label="Device limit" value={`${settings.downloads.activeDevices} of ${settings.downloads.deviceLimit} devices`} />
              <SettingsSaveBar saving={saving} status={status} onReset={() => setDraft(settings)} onSave={() => void saveSettings()} />
            </div>
          )}

          {section === "plan" && (
            <div className="ls-settings__section">
              <div className="ls-settings__sec-head">
                <div>
                  <h2 className="ls-settings__sec-title">Plan & billing</h2>
                  <p className="ls-settings__sec-sub">
                    Subscription, renewal, and payment method.
                  </p>
                </div>
              </div>
              <div className="ls-settings__plan">
                <div>
                  <div className="ls-settings__plan-title">{plan.planName}</div>
                  <div className="ls-settings__plan-meta mono">
                    {plan.features.join(" · ")}
                  </div>
                </div>
                <div className="ls-settings__plan-price">
                  ${plan.monthlyPrice.toFixed(2)}<span>/mo</span>
                </div>
              </div>
              <Field label="Next renewal" value={plan.nextRenewalDate} />
              <Field label="Payment method" value={`${plan.paymentBrand} •••• ${plan.paymentLast4}`} />
              <Field label="Billing address" value={`${plan.billingCity}, ${plan.billingRegion} · ${plan.billingCountry}`} />
              <Field label="Invoices" value={`${plan.invoicesCount} on file`} />
            </div>
          )}

          {section === "language" && (
            <div className="ls-settings__section">
              <div className="ls-settings__sec-head">
                <div>
                  <h2 className="ls-settings__sec-title">Language & region</h2>
                  <p className="ls-settings__sec-sub">
                    Interface language, subtitle preference, and catalog region.
                  </p>
                </div>
              </div>
              <Select label="Interface language" value={draft.language.interfaceLanguage} options={[draft.language.interfaceLanguage, "English (UK)", "Français", "Deutsch", "Español", "日本語", "한국어"]} onChange={(value) => setDraft({ ...draft, language: { ...draft.language, interfaceLanguage: value } })} />
              <Select label="Preferred subtitle language" value={draft.language.subtitleLanguage} options={[draft.language.subtitleLanguage, "French", "German", "Spanish", "Japanese", "Korean"]} onChange={(value) => setDraft({ ...draft, language: { ...draft.language, subtitleLanguage: value } })} />
              <Select label="Catalog region" value={draft.language.catalogRegion} options={[draft.language.catalogRegion, "United Kingdom", "Canada", "Germany", "Japan"]} onChange={(value) => setDraft({ ...draft, language: { ...draft.language, catalogRegion: value } })} />
              <Select label="Date format" value={draft.language.dateFormat} options={[draft.language.dateFormat, "D MMM YYYY", "YYYY-MM-DD"]} onChange={(value) => setDraft({ ...draft, language: { ...draft.language, dateFormat: value } })} />
              <Select label="24-hour clock" value={draft.language.clockFormat} options={[draft.language.clockFormat, "12 hour", "24 hour"]} onChange={(value) => setDraft({ ...draft, language: { ...draft.language, clockFormat: value } })} />
              <SettingsSaveBar saving={saving} status={status} onReset={() => setDraft(settings)} onSave={() => void saveSettings()} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  badge,
}: {
  readonly label: string;
  readonly value: string;
  readonly badge?: string;
}) {
  return (
    <div className="ls-settings__field">
      <div className="ls-settings__field-label mono">{label}</div>
      <div className="ls-settings__field-value">
        <span>{value}</span>
        {badge !== undefined && (
          <span className="ls-settings__field-badge mono">{badge}</span>
        )}
      </div>
    </div>
  );
}

function Select({
  label,
  value,
  options,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly options: ReadonlyArray<string>;
  readonly onChange: (value: string) => void;
}) {
  const uniqueOptions = Array.from(new Set(options));
  return (
    <div className="ls-settings__field">
      <div className="ls-settings__field-label mono">{label}</div>
      <div className="ls-settings__field-value">
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="ls-settings__select"
        >
          {uniqueOptions.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}

function Switch({
  label,
  on,
  onChange,
}: {
  readonly label: string;
  readonly on: boolean;
  readonly onChange: (value: boolean) => void;
}) {
  return (
    <label className="ls-settings__switch-row">
      <span className="ls-settings__field-label mono">{label}</span>
      <span className="ls-settings__switch">
        <input type="checkbox" checked={on} onChange={(e) => onChange(e.target.checked)} />
        <span className="ls-settings__switch-track">
          <span className="ls-settings__switch-dot" />
        </span>
      </span>
    </label>
  );
}

function ChannelRow({
  label,
  push,
  email,
  lock = false,
  onChange,
}: {
  readonly label: string;
  readonly push: boolean;
  readonly email: boolean;
  readonly lock?: boolean;
  readonly onChange: (value: { readonly push?: boolean; readonly email?: boolean }) => void;
}) {
  return (
    <div className="ls-settings__channel">
      <div className="ls-settings__field-label mono">
        {label}
        {lock && <Lock size={10} style={{ marginLeft: 6, verticalAlign: -1 }} />}
      </div>
      <div className="ls-settings__channel-controls">
        <SmallSwitch label="PUSH" on={push} onChange={(value) => onChange({ push: value })} locked={lock} />
        <SmallSwitch label="EMAIL" on={email} onChange={(value) => onChange({ email: value })} locked={lock} />
      </div>
    </div>
  );
}

function SettingsSaveBar({
  saving,
  status,
  onReset,
  onSave,
}: {
  readonly saving: boolean;
  readonly status: string | null;
  readonly onReset: () => void;
  readonly onSave: () => void;
}) {
  return (
    <div className="ls-settings__save">
      {status ? <span className="ls-settings__save-status">{status}</span> : <span />}
      <Button variant="outline" onClick={onReset} disabled={saving}>Reset</Button>
      <Button variant="primary" onClick={onSave} disabled={saving}>
        {saving ? "Saving…" : "Save changes"}
      </Button>
    </div>
  );
}

function SmallSwitch({
  label,
  on,
  onChange,
  locked,
}: {
  readonly label: string;
  readonly on: boolean;
  readonly onChange: (v: boolean) => void;
  readonly locked: boolean;
}) {
  return (
    <label className={`ls-settings__small-switch ${locked ? "is-locked" : ""}`}>
      <span className="mono">{label}</span>
      <span className="ls-settings__small-track">
        <input
          type="checkbox"
          checked={on}
          disabled={locked}
          onChange={(e) => onChange(e.target.checked)}
        />
        <span className="ls-settings__small-dot" />
      </span>
    </label>
  );
}
