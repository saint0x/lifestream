import { useState } from "react";
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

export function SettingsPage() {
  const user = useAppStore((s) => s.user);
  const profile = useAppStore((s) => s.profile);
  const settings = useAppStore((s) => s.settings);
  const plan = useAppStore((s) => s.plan);
  const [section, setSection] = useState<string>("account");

  return (
    <div className="ls-settings">
      <header className="ls-settings__head">
        <div className="ls-settings__kicker mono">/ yours / settings</div>
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
              onClick={() => setSection(key)}
            >
              <Icon size={14} strokeWidth={1.75} />
              <span>{label}</span>
            </button>
          ))}
          <div className="ls-settings__nav-sep" />
          <button type="button" className="ls-settings__nav-item ls-settings__nav-item--danger">
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
                <Button variant="outline" size="sm">Edit profile</Button>
              </div>

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
              <Field label="Password" value="•••••••••••" action="Change" />
              <Field
                label="Two-factor"
                value="Authenticator app"
                badge="enabled"
                action="Manage"
              />
              <Field
                label="Connected accounts"
                value={
                  profile.connectedAccounts.length > 0
                    ? profile.connectedAccounts.map((account) => account.displayName).join(", ")
                    : "None connected"
                }
                action="Manage"
              />
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
              <Select label="Default quality" value={settings.playback.defaultQuality} options={[settings.playback.defaultQuality, "1080p", "720p", "Data saver"]} />
              <Select label="Audio" value={settings.playback.audioLanguage} options={[settings.playback.audioLanguage, "Original language"]} />
              <Select label="Subtitles" value={settings.playback.subtitleStyle} options={[settings.playback.subtitleStyle, "Off"]} />
              <Switch label="Autoplay next episode" defaultOn={settings.playback.autoplayNextEpisode} />
              <Switch label="Autoplay trailers on hover" defaultOn={settings.playback.autoplayTrailers} />
              <Switch label="Reduced motion" defaultOn={settings.playback.reducedMotion} />
              <Switch label="Prefer dubbed over subtitles" defaultOn={settings.playback.preferDubbed} />
              <Select label="Playback speed default" value={settings.playback.playbackSpeed} options={[settings.playback.playbackSpeed, "1× (normal)", "1.25×", "1.5×", "1.75×", "2×"]} />
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
              <ChannelRow label={settings.notifications.seriesReleases.label} push={settings.notifications.seriesReleases.push} email={settings.notifications.seriesReleases.email} lock={settings.notifications.seriesReleases.lock} />
              <ChannelRow label={settings.notifications.liveStreams.label} push={settings.notifications.liveStreams.push} email={settings.notifications.liveStreams.email} lock={settings.notifications.liveStreams.lock} />
              <ChannelRow label={settings.notifications.originals.label} push={settings.notifications.originals.push} email={settings.notifications.originals.email} lock={settings.notifications.originals.lock} />
              <ChannelRow label={settings.notifications.watchlistUpdates.label} push={settings.notifications.watchlistUpdates.push} email={settings.notifications.watchlistUpdates.email} lock={settings.notifications.watchlistUpdates.lock} />
              <ChannelRow label={settings.notifications.creatorUpdates.label} push={settings.notifications.creatorUpdates.push} email={settings.notifications.creatorUpdates.email} lock={settings.notifications.creatorUpdates.lock} />
              <ChannelRow label={settings.notifications.securityAlerts.label} push={settings.notifications.securityAlerts.push} email={settings.notifications.securityAlerts.email} lock={settings.notifications.securityAlerts.lock} />
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
              <Switch label="Show my activity in 'what your friends are watching'" defaultOn={settings.privacy.showFriendActivity} />
              <Switch label="Use my viewing history to improve recommendations" defaultOn={settings.privacy.improveRecommendations} />
              <Switch label="Allow personalized ads on live streams" defaultOn={settings.privacy.personalizedAds} />
              <Switch label="Participate in anonymous A/B tests" defaultOn={settings.privacy.abTests} />
              <Field label="Download my data" value={`ZIP export, ~${settings.privacy.dataExportSizeMb} MB`} action="Request" />
              <Field label="Delete account" value={`Permanent · ${settings.privacy.deleteCooldownDays} day cooldown`} action="Delete" />
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
              <Select label="Maximum maturity rating" value={settings.parental.maxRating} options={[settings.parental.maxRating, "G", "PG", "PG-13", "TV-14", "TV-MA / R"]} />
              <Switch label="Require PIN for mature content" defaultOn={settings.parental.requirePinForMature} />
              <Switch label="Hide live chat for kids profiles" defaultOn={settings.parental.hideLiveChatForKids} />
              <Switch label="Block live streams tagged 'mature'" defaultOn={settings.parental.blockMatureLiveStreams} />
              <Field label="PIN" value="•••• " action="Change" />
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
              <Select label="Video quality" value={settings.downloads.videoQuality} options={[settings.downloads.videoQuality, "Standard (720p)", "High (1080p)", "Ultra (4K)"]} />
              <Switch label="Only download over Wi-Fi" defaultOn={settings.downloads.wifiOnly} />
              <Switch label="Smart downloads — keep next 3 episodes ready" defaultOn={settings.downloads.smartDownloads} />
              <Field label="Storage used" value={`${settings.downloads.storageUsedGb} GB of ${settings.downloads.storageLimitGb} GB`} action="Clear" />
              <Field label="Device limit" value={`${settings.downloads.activeDevices} of ${settings.downloads.deviceLimit} devices`} action="Manage" />
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
              <Field label="Payment method" value={`${plan.paymentBrand} •••• ${plan.paymentLast4}`} action="Update" />
              <Field label="Billing address" value={`${plan.billingCity}, ${plan.billingRegion} · ${plan.billingCountry}`} action="Edit" />
              <Field label="Invoices" value={`${plan.invoicesCount} on file`} action="Download" />
              <div className="ls-settings__plan-actions">
                <Button variant="ghost">Downgrade</Button>
                <Button variant="ghost">Pause membership</Button>
                <Button variant="ghost">Cancel subscription</Button>
              </div>
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
              <Select label="Interface language" value={settings.language.interfaceLanguage} options={[settings.language.interfaceLanguage, "English (UK)", "Français", "Deutsch", "Español", "日本語", "한국어"]} />
              <Select label="Preferred subtitle language" value={settings.language.subtitleLanguage} options={[settings.language.subtitleLanguage, "French", "German", "Spanish", "Japanese", "Korean"]} />
              <Select label="Catalog region" value={settings.language.catalogRegion} options={[settings.language.catalogRegion, "United Kingdom", "Canada", "Germany", "Japan"]} />
              <Select label="Date format" value={settings.language.dateFormat} options={[settings.language.dateFormat, "D MMM YYYY", "YYYY-MM-DD"]} />
              <Select label="24-hour clock" value={settings.language.clockFormat} options={[settings.language.clockFormat, "12 hour", "24 hour"]} />
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
  action,
  badge,
}: {
  readonly label: string;
  readonly value: string;
  readonly action?: string;
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
      {action !== undefined && (
        <button type="button" className="ls-settings__field-action">
          {action}
        </button>
      )}
    </div>
  );
}

function Select({
  label,
  value,
  options,
}: {
  readonly label: string;
  readonly value: string;
  readonly options: ReadonlyArray<string>;
}) {
  const [current, setCurrent] = useState(value);
  return (
    <div className="ls-settings__field">
      <div className="ls-settings__field-label mono">{label}</div>
      <div className="ls-settings__field-value">
        <select
          value={current}
          onChange={(e) => setCurrent(e.target.value)}
          className="ls-settings__select"
        >
          {options.map((o) => (
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
  defaultOn = false,
}: {
  readonly label: string;
  readonly defaultOn?: boolean;
}) {
  const [on, setOn] = useState(defaultOn);
  return (
    <label className="ls-settings__switch-row">
      <span className="ls-settings__field-label mono">{label}</span>
      <span className="ls-settings__switch">
        <input type="checkbox" checked={on} onChange={(e) => setOn(e.target.checked)} />
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
}: {
  readonly label: string;
  readonly push: boolean;
  readonly email: boolean;
  readonly lock?: boolean;
}) {
  const [p, setP] = useState(push);
  const [e, setE] = useState(email);
  return (
    <div className="ls-settings__channel">
      <div className="ls-settings__field-label mono">
        {label}
        {lock && <Lock size={10} style={{ marginLeft: 6, verticalAlign: -1 }} />}
      </div>
      <div className="ls-settings__channel-controls">
        <SmallSwitch label="PUSH" on={p} onChange={setP} locked={lock} />
        <SmallSwitch label="EMAIL" on={e} onChange={setE} locked={lock} />
      </div>
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
