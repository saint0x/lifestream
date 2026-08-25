import { useState } from "react";
import {
  Activity,
  BadgeCheck,
  FileVideo,
  Hash,
  Headphones,
  Eye,
  Keyboard,
  Layers,
  MessageSquareWarning,
  MicOff,
  PackageCheck,
  RadioTower,
  Scissors,
  ShieldAlert,
  SlidersHorizontal,
  Sparkles,
  TrendingUp,
  Tv,
  UserPlus,
  Users,
  Wand2,
} from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { sourceBadgeTone, sourceFilterSummary, sourceSyncState } from "@/engine/sourceSync";
import { runtimeSocketTone, type RuntimeSocketState } from "@/engine/runtime";
import {
  transitionLabel,
  transitionPhaseSummary,
  transitionPlanFromPreview,
  transitionRenderer,
} from "@/engine/transitions";
import { formatDuration } from "@/lib/format";
import type { ObsRow } from "@/types";
import { boolish, jsonArray, num, text } from "@/types";
import { Panel } from "./Panel";

export function TransitionPanel({
  scene,
  runtime,
  preview,
  onSend,
  onPreview,
  onPreflight,
}: {
  readonly scene: ObsRow | null;
  readonly runtime: ObsRow;
  readonly preview: ObsRow | null;
  readonly onSend: () => void;
  readonly onPreview: () => void;
  readonly onPreflight: () => void;
}) {
  const latestTransition = objectValue(runtime.latest_transition_json);
  const previewPlan = transitionPlanFromPreview(preview);
  const latestPlan = objectValue(latestTransition.preview_json);
  const displayedPlan = Object.keys(previewPlan).length ? previewPlan : latestPlan;
  const phases = transitionPhaseSummary(displayedPlan);
  return (
    <Panel
      title="Transition"
      icon={<Wand2 />}
      summary={<><strong>{text(scene, "transition_kind", "fade")}</strong><span>{num(scene, "transition_duration_ms")}ms</span></>}
      defaultCollapsed
    >
      <div className="obs-transition">
        <div>
          <strong>{text(scene, "transition_kind", "fade")}</strong>
          <span className="mono">{num(scene, "transition_duration_ms")}ms / studio mode</span>
        </div>
        {latestTransition.id ? (
          <div className="obs-transition__latest mono">
            <span>{stringValue(latestTransition.status, "completed")}</span>
            <strong>
              {stringValue(latestTransition.kind, stringValue(latestTransition.transition_kind, "cut"))} / {numberValue(latestTransition.duration_ms)}ms
            </strong>
          </div>
        ) : null}
        {Object.keys(displayedPlan).length ? (
          <div className="obs-transition__preview">
            <div className="obs-transition__latest mono">
              <span>{previewPlan.kind ? "Preview" : "Program"}</span>
              <strong>{transitionLabel(displayedPlan)} / {transitionRenderer(displayedPlan)}</strong>
            </div>
            <div className="obs-transition__phases">
              {phases.slice(0, 4).map((phase) => (
                <Badge key={phase} tone="neutral">{phase}</Badge>
              ))}
            </div>
          </div>
        ) : null}
        <Button variant="secondary" icon={<Eye />} onClick={onPreview} full>Preview</Button>
        <Button variant="primary" icon={<Tv />} onClick={onSend} full>Send Program</Button>
        <Button variant="secondary" icon={<ShieldAlert />} onClick={onPreflight} full>Preflight</Button>
      </div>
    </Panel>
  );
}

export function AudioMixer({
  channels,
  onPatch,
}: {
  readonly channels: readonly ObsRow[];
  readonly onPatch: (channel: ObsRow, patch: Record<string, unknown>) => void;
}) {
  const activeChannels = channels.filter((channel) => !boolish(channel, "muted")).length;
  const mix = objectValue(channels[0]?.audio_mix_json);
  const drift = objectValue(mix.drift_correction);
  return (
    <Panel
      title="Audio"
      icon={<SlidersHorizontal />}
      summary={<><strong>{activeChannels}/{channels.length} live</strong><span>{stringValue(drift.status, "locked")}</span></>}
      defaultCollapsed
    >
      <div className="obs-mixer__channels">
        {channels.map((channel) => {
          const graph = audioGraph(channel);
          const meter = objectValue(graph.meter);
          const buses = objectValue(graph.buses);
          const warnings = Array.isArray(graph.warnings) ? graph.warnings : [];
          const level = numberValue(meter.level_percent);
          return (
            <div className="obs-channel" key={channel.id}>
              <div className="obs-channel__meter"><span style={{ height: `${level}%` }} /></div>
              <strong>{text(channel, "label")}</strong>
              <em className="mono">
                {num(channel, "gain_db")} dB / {numberValue(meter.peak_db)} pk
              </em>
              <Badge tone={warnings.length > 0 ? "premium" : boolish(channel, "muted") ? "neutral" : "hd"}>
                {warnings[0] ? String(warnings[0]) : boolish(channel, "muted") ? "muted" : "program"}
              </Badge>
              <div className="obs-channel__controls">
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<MicOff />}
                  aria-label={boolish(channel, "muted") ? "Unmute" : "Mute"}
                  onClick={() => onPatch(channel, { muted: !boolish(channel, "muted") })}
                />
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<Headphones />}
                  aria-label={boolish(channel, "monitor_enabled") ? "Disable monitor" : "Enable monitor"}
                  onClick={() => onPatch(channel, { monitor_enabled: !boolish(channel, "monitor_enabled") })}
                />
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onPatch(channel, { solo: !boolish(channel, "solo") })}
                >
                  Solo
                </Button>
              </div>
              <em className="mono">
                {boolValue(buses.monitor) ? "MON" : "PGM"} / {boolValue(buses.mix_minus) ? "MIX-" : "MAIN"}
              </em>
              <em className="mono">
                {stringValue(objectValue(graph.drift_correction).status, "standby")} / {numberValue(objectValue(graph.drift_correction).residual_drift_ms)}ms
              </em>
            </div>
          );
        })}
      </div>
    </Panel>
  );
}

function audioGraph(channel: ObsRow): Record<string, unknown> {
  return objectValue(channel.audio_graph_json);
}

function objectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function numberValue(value: unknown): number {
  return typeof value === "number" ? value : 0;
}

function stringValue(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim() ? value : fallback;
}

function boolValue(value: unknown): boolean {
  return value === true || value === 1;
}

function stringArrayValue(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function fallbackLabel(status: unknown): string {
  switch (status) {
    case "native_ready":
      return "native ready";
    case "browser_preview_external_ingest":
      return "fallback ready";
    default:
      return "pending";
  }
}

function objectArrayValue(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value)
    ? value.filter((item): item is Record<string, unknown> => (
        item !== null && typeof item === "object" && !Array.isArray(item)
      ))
    : [];
}

function money(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}

export function Inspector({
  source,
  onPatchSource,
  onCreateFilter,
  onPatchFilter,
  onDisableFilter,
}: {
  readonly source: ObsRow | null;
  readonly onPatchSource: (sourceId: string, patch: Record<string, unknown>) => void;
  readonly onCreateFilter: (sourceId: string) => void;
  readonly onPatchFilter: (filterId: string, patch: Record<string, unknown>) => void;
  readonly onDisableFilter: (filterId: string) => void;
}) {
  const sync = sourceSyncState(source);
  const filters = jsonArray(source, "filters_chain_json");
  const settings = objectValue(source?.default_settings_json);
  const editorFields = sourceEditorFields(text(source, "source_kind"));
  return (
    <Panel
      title="Inspector"
      icon={<SlidersHorizontal />}
      summary={<><strong>{text(source, "source_kind", "none")}</strong><span>{sync.status}</span></>}
      defaultCollapsed
    >
      <div className="obs-inspector">
        <Input
          key={`${source?.id ?? "none"}-display`}
          defaultValue={text(source, "display_name")}
          readOnly={!source}
          onBlur={(event) => {
            const next = event.currentTarget.value.trim();
            if (source && next && next !== text(source, "display_name")) {
              onPatchSource(source.id, { display_name: next });
            }
          }}
        />
        <div className="obs-kv mono"><span>Kind</span><strong>{text(source, "source_kind")}</strong></div>
        <div className="obs-kv mono"><span>Renderer</span><strong>{sync.renderer}</strong></div>
        <div className="obs-kv mono"><span>Permission</span><strong>{sync.permissionRequired ? sync.permissionKind : "inline"}</strong></div>
        <div className="obs-kv mono"><span>Sync</span><Badge tone={source ? sourceBadgeTone(source) : "neutral"}>{sync.status}</Badge></div>
        <div className="obs-kv mono"><span>OBS</span><strong>{sync.obsKind || "native"}</strong></div>
        {text(source, "device_id") ? <div className="obs-kv mono"><span>Device</span><strong>{text(source, "device_id")}</strong></div> : null}
        {text(source, "browser_url") ? <div className="obs-kv mono"><span>URL</span><strong>{text(source, "browser_url")}</strong></div> : null}
        {text(source, "media_asset_id") ? <div className="obs-kv mono"><span>Asset</span><strong>{text(source, "media_asset_id")}</strong></div> : null}
        {sync.issues.slice(0, 2).map((issue) => (
          <div className="obs-kv mono" key={issue}><span>Issue</span><strong>{issue}</strong></div>
        ))}
        {editorFields.length ? (
          <div className="obs-inspector__editor">
            {editorFields.map((field) => (
              <label className="obs-field" key={`${source?.id ?? "none"}-${field.key}`}>
                <span className="mono">{field.label}</span>
                <Input
                  key={`${source?.id ?? "none"}-${field.key}`}
                  type={field.kind === "number" ? "number" : field.kind === "color" ? "color" : "text"}
                  defaultValue={settingsText(settings, field.key)}
                  readOnly={!source}
                  onBlur={(event) => {
                    if (!source) return;
                    const next = parseSettingsValue(event.currentTarget.value, field.kind);
                    if (next === settings[field.key]) return;
                    onPatchSource(source.id, {
                      settings_json: {
                        ...settings,
                        [field.key]: next,
                      },
                    });
                  }}
                />
              </label>
            ))}
          </div>
        ) : null}
        <div className="obs-inspector__filters">
          <div className="obs-kv mono">
            <span>Filters</span>
            <Button size="sm" variant="ghost" onClick={() => source && onCreateFilter(source.id)} disabled={!source}>
              Add
            </Button>
          </div>
          {filters.slice(0, 4).map((filter) => {
            const enabled = boolish(filter, "enabled");
            return (
              <div className="obs-filter" key={filter.id}>
                <span>
                  <strong>{text(filter, "label")}</strong>
                  <em className="mono">{sourceFilterSummary(filter)}</em>
                </span>
                <Badge tone={enabled ? "hd" : "neutral"}>{enabled ? "on" : "off"}</Badge>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onPatchFilter(filter.id, { enabled: !enabled })}
                >
                  {enabled ? "Off" : "On"}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => onDisableFilter(filter.id)}>
                  Disable
                </Button>
              </div>
            );
          })}
        </div>
      </div>
    </Panel>
  );
}

type SourceEditorFieldKind = "text" | "number" | "color";

interface SourceEditorField {
  readonly key: string;
  readonly label: string;
  readonly kind: SourceEditorFieldKind;
}

function sourceEditorFields(sourceKind: string): readonly SourceEditorField[] {
  switch (sourceKind) {
    case "browser_capture":
      return [field("width", "Width", "number"), field("height", "Height", "number")];
    case "media_file":
    case "vanta_video_asset":
    case "vanta_clip":
      return [field("media_url", "Media URL"), field("media_path", "Media Path")];
    case "image":
      return [field("image_url", "Image URL"), field("media_path", "Image Path")];
    case "text":
      return [field("text", "Text")];
    case "lower_third":
      return [field("headline", "Headline"), field("subhead", "Subhead")];
    case "branded_bumper":
      return [field("headline", "Headline")];
    case "pinned_cta":
      return [field("cta_text", "CTA"), field("target_url", "Target")];
    case "qr_code":
      return [field("target_url", "Target")];
    case "promo_code":
      return [field("promo_code", "Code")];
    case "sponsor_card":
      return [field("brand", "Brand"), field("promo_code", "Code")];
    case "countdown_timer":
      return [field("seconds", "Seconds", "number")];
    case "chat_overlay":
    case "alert_overlay":
      return [field("feed_label", "Feed")];
    case "guest_feed":
    case "remote_contribution":
      return [field("participant_label", "Guest")];
    case "color_matte":
      return [field("color", "Color", "color")];
    default:
      return [];
  }
}

function field(key: string, label: string, kind: SourceEditorFieldKind = "text"): SourceEditorField {
  return { key, label, kind };
}

function settingsText(settings: Record<string, unknown>, key: string): string | number | undefined {
  const value = settings[key];
  return typeof value === "string" || typeof value === "number" ? value : undefined;
}

function parseSettingsValue(value: string, kind: SourceEditorFieldKind): string | number {
  if (kind === "number") return Number(value) || 0;
  return value;
}

export function SceneGroupsPanel({
  scene,
  scenes,
  sources,
  instances,
  onCreate,
  onPatch,
}: {
  readonly scene: ObsRow | null;
  readonly scenes: readonly ObsRow[];
  readonly sources: readonly ObsRow[];
  readonly instances: readonly ObsRow[];
  readonly onCreate: (childSceneId: string) => void;
  readonly onPatch: (sourceId: string, childSceneId: string) => void;
}) {
  const childOptions = scenes.filter((item) => item.id !== scene?.id);
  const firstChildId = childOptions[0]?.id ?? "";
  const groupInstances = instances.filter((instance) => text(instance, "scene_id") === scene?.id)
    .map((instance) => {
      const source = sources.find((item) => item.id === text(instance, "source_id"));
      return source && text(source, "source_kind") === "scene_group" ? { instance, source } : null;
    })
    .filter((item): item is { instance: ObsRow; source: ObsRow } => item !== null);
  return (
    <Panel
      title="Scene Groups"
      icon={<Layers />}
      summary={<strong>{groupInstances.length}</strong>}
      defaultCollapsed
    >
      <div className="obs-groups">
        <div className="obs-template-bar">
          <select
            className="obs-template-bar__select mono"
            aria-label="Nested scene"
            defaultValue={firstChildId}
            disabled={!scene || childOptions.length === 0}
            onChange={(event) => {
              event.currentTarget.dataset.childSceneId = event.target.value;
            }}
          >
            {childOptions.map((option) => (
              <option key={option.id} value={option.id}>{text(option, "name")}</option>
            ))}
          </select>
          <Button
            size="sm"
            variant="secondary"
            disabled={!scene || !firstChildId}
            onClick={(event) => {
              const select = event.currentTarget.parentElement?.querySelector("select");
              onCreate(select?.dataset.childSceneId || firstChildId);
            }}
          >
            Add
          </Button>
        </div>
        {groupInstances.slice(0, 4).map(({ source }) => {
          const settings = objectValue(source.default_settings_json);
          const childSceneId = stringValue(settings.scene_id, "");
          const child = scenes.find((item) => item.id === childSceneId);
          return (
            <div className="obs-group" key={source.id}>
              <span>
                <strong>{text(source, "display_name")}</strong>
                <em className="mono">{text(child, "name", childSceneId)}</em>
              </span>
              <Badge tone="hd">nested</Badge>
              <select
                className="obs-template-bar__select mono"
                aria-label="Retarget nested scene"
                value={childSceneId}
                onChange={(event) => onPatch(source.id, event.target.value)}
              >
                {childOptions.map((option) => (
                  <option key={option.id} value={option.id}>{text(option, "name")}</option>
                ))}
              </select>
            </div>
          );
        })}
      </div>
    </Panel>
  );
}

export function HealthPanel({ health, preflight }: { readonly health: ObsRow; readonly preflight: ObsRow }) {
  const checks = jsonArray(preflight, "checks_json");
  const fallback = objectValue(health.native_fallback_json);
  const externalIngest = objectValue(fallback.external_ingest);
  const blockedHelpers = Array.isArray(fallback.blocked_helpers) ? fallback.blocked_helpers : [];
  return (
    <Panel
      title="Health"
      icon={<Activity />}
      summary={<><strong>{text(health, "status")}</strong><span>{checks.length} checks</span><span>{fallbackLabel(fallback.status)}</span></>}
      defaultCollapsed
    >
      <div className="obs-health">
        <div className="obs-health__hero">
          <strong>{text(health, "status")}</strong>
          <span className="mono">{num(health, "upload_mbps")} mbps / {num(health, "ingest_latency_ms")}ms</span>
        </div>
        {checks.slice(0, 4).map((check) => (
          <div className="obs-check mono" key={check.id || text(check, "key")}>
            <span>{text(check, "label")}</span>
            <Badge tone={text(check, "status") === "pass" ? "hd" : "premium"}>{text(check, "status")}</Badge>
          </div>
        ))}
        <div className="obs-check mono">
          <span>Native fallback</span>
          <Badge tone={boolValue(fallback.native_ready) ? "hd" : "premium"}>
            {fallbackLabel(fallback.status)}
          </Badge>
        </div>
        <div className="obs-check mono">
          <span>External ingest</span>
          <strong>{boolValue(externalIngest.available) ? "ready" : "pending"} / {blockedHelpers.length} blockers</strong>
        </div>
      </div>
    </Panel>
  );
}

export function ChannelPanel({
  broadcast,
  runtime,
  onPatch,
}: {
  readonly broadcast: ObsRow;
  readonly runtime: ObsRow;
  readonly onPatch: (patch: Record<string, unknown>) => void;
}) {
  const channel = objectValue(objectValue(runtime.runtime_status_json).channel);
  const [draft, setDraft] = useState({
    title: text(broadcast, "title"),
    category: text(broadcast, "category"),
    tags: stringArrayValue(channel.tags).join(", "),
    chatMode: text(broadcast, "chat_mode"),
    mature: boolish(broadcast, "mature_content"),
  });
  return (
    <Panel
      title="Channel"
      icon={<Hash />}
      summary={<><strong>{text(broadcast, "category")}</strong><span>{draft.chatMode}</span></>}
      defaultCollapsed
    >
      <div className="obs-channel-panel">
        <Input
          value={draft.title}
          aria-label="Stream title"
          onChange={(event) => setDraft((current) => ({ ...current, title: event.currentTarget.value }))}
        />
        <div className="obs-template-bar">
          <Input
            value={draft.category}
            aria-label="Category"
            onChange={(event) => setDraft((current) => ({ ...current, category: event.currentTarget.value }))}
          />
          <select
            className="obs-template-bar__select mono"
            aria-label="Chat mode"
            value={draft.chatMode}
            onChange={(event) => setDraft((current) => ({ ...current, chatMode: event.currentTarget.value }))}
          >
            <option value="open">open</option>
            <option value="slow_mode">slow</option>
            <option value="subscriber_only">subs</option>
            <option value="follower_only">followers</option>
            <option value="subscriber_slow_mode">subs slow</option>
          </select>
        </div>
        <Input
          value={draft.tags}
          aria-label="Tags"
          onChange={(event) => setDraft((current) => ({ ...current, tags: event.currentTarget.value }))}
        />
        <div className="obs-kv mono">
          <span>{stringValue(channel.visibility, text(broadcast, "visibility"))}</span>
          <Badge tone={boolValue(channel.follower_notification) ? "hd" : "neutral"}>
            {boolValue(channel.follower_notification) ? "notify" : "quiet"}
          </Badge>
        </div>
        <div className="obs-channel-panel__actions">
          <Button
            size="sm"
            variant={draft.mature ? "danger" : "secondary"}
            onClick={() => setDraft((current) => ({ ...current, mature: !current.mature }))}
          >
            Mature
          </Button>
          <Button
            size="sm"
            variant="primary"
            onClick={() => onPatch({
              title: draft.title,
              category: draft.category,
              tags: draft.tags.split(",").map((tag) => tag.trim()).filter(Boolean),
              chat_mode: draft.chatMode,
              mature_content: draft.mature,
            })}
          >
            Save
          </Button>
        </div>
      </div>
    </Panel>
  );
}

export function ModerationPanel({
  moderation,
  onBlockTerm,
  onModerator,
  onQueue,
  onResolve,
  onPin,
  onUnpin,
}: {
  readonly moderation: ObsRow;
  readonly onBlockTerm: () => void;
  readonly onModerator: () => void;
  readonly onQueue: () => void;
  readonly onResolve: (itemId: string, status: "approved" | "hidden" | "banned") => void;
  readonly onPin: () => void;
  readonly onUnpin: (messageId: string) => void;
}) {
  const queue = jsonArray(moderation, "queue_json");
  const blocked = jsonArray(moderation, "blocked_terms_json");
  const moderators = jsonArray(moderation, "moderators_json");
  const pins = jsonArray(moderation, "pinned_messages_json");
  const activePin = pins.find((pin) => text(pin, "status") === "active");
  return (
    <Panel
      title="Moderation"
      icon={<MessageSquareWarning />}
      summary={<><strong>{num(moderation, "pending_count")} queue</strong><span>{pins.length} pin</span></>}
      defaultCollapsed
    >
      <div className="obs-moderation">
        <div className="obs-kv mono">
          <span>Queue</span>
          <Badge tone={num(moderation, "pending_count") > 0 ? "premium" : "hd"}>
            {num(moderation, "pending_count")}
          </Badge>
        </div>
        {activePin ? (
          <div className="obs-event">
            <MessageSquareWarning size={13} />
            <span>{text(activePin, "message")}</span>
            <Button size="sm" variant="ghost" onClick={() => onUnpin(activePin.id)}>Unpin</Button>
          </div>
        ) : null}
        {queue.slice(0, 2).map((item) => (
          <div className="obs-filter" key={item.id}>
            <span>
              <strong>{text(item, "author_name")}</strong>
              <em className="mono">{text(item, "reason")}</em>
            </span>
            <Badge tone={text(item, "status") === "pending" ? "premium" : "neutral"}>{text(item, "status")}</Badge>
            <Button size="sm" variant="ghost" onClick={() => onResolve(item.id, "approved")}>Approve</Button>
            <Button size="sm" variant="ghost" onClick={() => onResolve(item.id, "hidden")}>Hide</Button>
          </div>
        ))}
        <div className="obs-kv mono">
          <span>Terms</span>
          <strong>{blocked.map((term) => text(term, "term")).slice(0, 3).join(", ") || "clear"}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Mods</span>
          <strong>{moderators.map((mod) => text(mod, "display_name")).slice(0, 2).join(", ") || "none"}</strong>
        </div>
        <div className="obs-moderation__actions">
          <Button size="sm" variant="secondary" onClick={onQueue}>Queue</Button>
          <Button size="sm" variant="secondary" onClick={onBlockTerm}>Term</Button>
          <Button size="sm" variant="secondary" onClick={onModerator}>Mod</Button>
          <Button size="sm" variant="secondary" onClick={onPin}>Pin</Button>
        </div>
      </div>
    </Panel>
  );
}

export function AudiencePanel({
  audience,
  onSample,
  onRaid,
  onInboundRaid,
}: {
  readonly audience: ObsRow;
  readonly onSample: () => void;
  readonly onRaid: () => void;
  readonly onInboundRaid: () => void;
}) {
  const latest = objectValue(audience.latest_snapshot);
  const outboundRaid = objectValue(audience.latest_outbound_raid);
  const inboundRaid = objectValue(audience.latest_inbound_raid);
  return (
    <Panel
      title="Audience"
      icon={<TrendingUp />}
      summary={<><strong>{num(audience, "viewer_count").toLocaleString()}</strong><span>{formatDuration(num(audience, "uptime_seconds"))}</span></>}
      defaultCollapsed
    >
      <div className="obs-audience">
        <div className="obs-audience__hero">
          <strong>{num(audience, "viewer_count").toLocaleString()}</strong>
          <span className="mono">{formatDuration(num(audience, "uptime_seconds"))} live</span>
        </div>
        <div className="obs-kv mono">
          <span>Peak / avg</span>
          <strong>{num(audience, "peak_viewers")} / {num(audience, "average_viewers")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Chat</span>
          <strong>{num(audience, "chat_messages_per_minute")}/min</strong>
        </div>
        <div className="obs-kv mono">
          <span>Revenue</span>
          <strong>{money(num(audience, "revenue_cents"))} / {num(audience, "subscriptions")} subs</strong>
        </div>
        <div className="obs-kv mono">
          <span>{text(audience, "discovery_source", "pending")}</span>
          <Badge tone={num(audience, "discovery_score") >= 75 ? "hd" : "neutral"}>
            {num(audience, "discovery_score")}
          </Badge>
        </div>
        <div className="obs-kv mono">
          <span>Latest</span>
          <strong>{money(numberValue(latest.tips_cents))} tips</strong>
        </div>
        {outboundRaid.id ? (
          <div className="obs-kv mono">
            <span>Redirect</span>
            <strong>{stringValue(outboundRaid.status, "scheduled")} {numberValue(outboundRaid.viewer_count)}</strong>
          </div>
        ) : null}
        {inboundRaid.id ? (
          <div className="obs-kv mono">
            <span>Raid in</span>
            <strong>{stringValue(inboundRaid.target_channel_name, "channel")} {numberValue(inboundRaid.viewer_count)}</strong>
          </div>
        ) : null}
        <div className="obs-audience__actions">
          <Button size="sm" variant="secondary" icon={<TrendingUp />} onClick={onSample}>
            Sample
          </Button>
          <Button size="sm" variant="secondary" onClick={onRaid}>Raid</Button>
          <Button size="sm" variant="ghost" onClick={onInboundRaid}>In</Button>
        </div>
      </div>
    </Panel>
  );
}

export function EngagementPanel({
  engagement,
  onSchedule,
  onReschedule,
  onPoll,
  onPrediction,
  onVote,
  onClosePoll,
  onAlert,
}: {
  readonly engagement: ObsRow;
  readonly onSchedule: () => void;
  readonly onReschedule: (slotId: string) => void;
  readonly onPoll: () => void;
  readonly onPrediction: () => void;
  readonly onVote: (pollId: string, optionId: string) => void;
  readonly onClosePoll: (pollId: string) => void;
  readonly onAlert: () => void;
}) {
  const nextSlot = objectValue(engagement.next_slot);
  const activePoll = objectValue(engagement.active_poll);
  const pollOptions = objectArrayValue(activePoll.options_json);
  const alerts = jsonArray(engagement, "alerts_json");
  return (
    <Panel
      title="Engagement"
      icon={<RadioTower />}
      summary={<><strong>{activePoll.id ? "poll live" : stringValue(nextSlot.status, "idle")}</strong><span>{alerts.length} alerts</span></>}
      defaultCollapsed
    >
      <div className="obs-engagement">
        <div className="obs-kv mono">
          <span>{stringValue(nextSlot.status, "schedule")}</span>
          <strong>{stringValue(nextSlot.title, "none")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>{stringValue(nextSlot.timezone, "timezone")}</span>
          <strong>{stringValue(nextSlot.starts_at, "unscheduled")}</strong>
        </div>
        {activePoll.id ? (
          <div className="obs-engagement__poll">
            <div className="obs-kv mono">
              <span>{stringValue(activePoll.poll_kind, "poll")}</span>
              <Badge tone={boolValue(activePoll.is_prediction) ? "premium" : "hd"}>
                {numberValue(activePoll.total_votes)}
              </Badge>
            </div>
            <strong>{stringValue(activePoll.question, "Live poll")}</strong>
            {pollOptions.slice(0, 3).map((option) => (
              <button
                className="obs-engagement__option mono"
                key={String(option.id)}
                onClick={() => onVote(String(activePoll.id), String(option.id))}
              >
                <span>{stringValue(option.label, "")}</span>
                <strong>{numberValue(option.votes)} / {numberValue(option.percent)}%</strong>
              </button>
            ))}
            <Button size="sm" variant="ghost" onClick={() => onClosePoll(String(activePoll.id))}>Close</Button>
          </div>
        ) : null}
        {alerts.slice(0, 2).map((alert) => (
          <div className="obs-event" key={alert.id}>
            <RadioTower size={13} />
            <span>{text(alert, "title")} / {money(num(alert, "amount_cents"))}</span>
          </div>
        ))}
        <div className="obs-engagement__actions">
          <Button size="sm" variant="secondary" onClick={onSchedule}>Schedule</Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={!nextSlot.id}
            onClick={() => onReschedule(String(nextSlot.id))}
          >
            Shift
          </Button>
          <Button size="sm" variant="secondary" onClick={onPoll}>Poll</Button>
          <Button size="sm" variant="secondary" onClick={onPrediction}>Predict</Button>
          <Button size="sm" variant="secondary" onClick={onAlert}>Alert</Button>
        </div>
      </div>
    </Panel>
  );
}

export function SponsorPanel({
  sponsor,
  onAttach,
  onInventory,
  onProof,
  onReview,
}: {
  readonly sponsor: ObsRow;
  readonly onAttach: () => void;
  readonly onInventory: (creativeKind: "sponsor_card" | "lower_third" | "branded_bumper" | "pinned_cta" | "qr_code" | "promo_code") => void;
  readonly onProof: (inventoryId: string) => void;
  readonly onReview: (proofId: string) => void;
}) {
  const campaign = objectValue(sponsor.active_campaign);
  const inventory = jsonArray(sponsor, "inventory_json");
  const proofs = jsonArray(sponsor, "proofs_json");
  const next = objectValue(sponsor.next_inventory);
  const latestProof = proofs[0] ?? null;
  const latestProofArtifact = objectValue(latestProof?.artifact_json);
  const latestProofAsset = objectValue(latestProof?.vanta_asset_json);
  const handoff = objectValue(sponsor.performance_handoff_json);
  const proofMediaStatus = stringValue(
    latestProofAsset.status,
    latestProofArtifact.validation ? "ready" : "pending",
  );
  return (
    <Panel
      title="Sponsor"
      icon={<Sparkles />}
      summary={<><strong>{stringValue(campaign.advertiser, "none")}</strong><span>{proofMediaStatus} {num(sponsor, "approved_proof_count")}/{num(sponsor, "proof_count")}</span></>}
      defaultCollapsed
    >
      <div className="obs-sponsor">
        <div className="obs-kv mono">
          <span>{stringValue(campaign.status, "campaign")}</span>
          <strong>{stringValue(campaign.advertiser, "none")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Next</span>
          <strong>{stringValue(next.label, "none")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Proofs</span>
          <strong>{num(sponsor, "approved_proof_count")} / {num(sponsor, "proof_count")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Missed</span>
          <Badge tone={num(sponsor, "missed_count") > 0 ? "premium" : "hd"}>
            {num(sponsor, "missed_count")}
          </Badge>
        </div>
        <div className="obs-kv mono">
          <span>Handoff</span>
          <strong>{stringValue(handoff.handoff, "pending")}</strong>
        </div>
        {inventory.slice(0, 3).map((item) => (
          <div className="obs-filter" key={item.id}>
            <span>
              <strong>{text(item, "label")}</strong>
              <em className="mono">{text(item, "creative_kind")} / {text(item, "status")}</em>
            </span>
            <Badge tone={text(item, "review_status") === "approved" ? "hd" : "premium"}>
              {text(item, "review_status")}
            </Badge>
            <Button size="sm" variant="ghost" onClick={() => onProof(item.id)}>Proof</Button>
          </div>
        ))}
        {latestProof ? (
          <div className="obs-event">
            <Sparkles size={13} />
            <span>
              {text(latestProof, "proof_kind")} / {proofMediaStatus} / {stringValue(latestProofArtifact.source_kind, "media")}
            </span>
            <Button size="sm" variant="ghost" onClick={() => onReview(latestProof.id)}>Review</Button>
          </div>
        ) : null}
        <div className="obs-sponsor__actions">
          <Button size="sm" variant="secondary" onClick={onAttach}>Campaign</Button>
          <Button size="sm" variant="secondary" onClick={() => onInventory("sponsor_card")}>Card</Button>
          <Button size="sm" variant="secondary" onClick={() => onInventory("lower_third")}>Lower</Button>
          <Button size="sm" variant="secondary" onClick={() => onInventory("qr_code")}>QR</Button>
          <Button size="sm" variant="secondary" onClick={() => onInventory("promo_code")}>Promo</Button>
          <Button size="sm" variant="secondary" onClick={() => onInventory("pinned_cta")}>CTA</Button>
        </div>
      </div>
    </Panel>
  );
}

export function SafetyPanel({
  safety,
  onEmergencyDisconnect,
  onLiveOpsOverride,
  onSupportBundle,
}: {
  readonly safety: ObsRow;
  readonly onEmergencyDisconnect: () => void;
  readonly onLiveOpsOverride: (action: "force_end" | "safe_mode" | "clear_incidents") => void;
  readonly onSupportBundle: () => void;
}) {
  const incident = objectValue(safety.latest_incident);
  const bundle = objectValue(safety.latest_support_bundle);
  const guards = objectValue(safety.action_guards_json);
  const streamEndGuard = objectValue(guards.stream_end);
  const recordingGuard = objectValue(guards.recording_stop);
  const forceGuard = objectValue(guards.force_end);
  const [armedAction, setArmedAction] = useState<"force_end" | null>(null);
  return (
    <Panel
      title="Safety"
      icon={<ShieldAlert />}
      summary={<><strong>{boolish(safety, "preflight_ready") ? "ready" : "blocked"}</strong><span>{stringValue(incident.incident_kind, "clear")}</span></>}
      defaultCollapsed
    >
      <div className="obs-safety">
        <div className="obs-kv mono">
          <span>Preflight</span>
          <Badge tone={boolish(safety, "preflight_ready") ? "hd" : "premium"}>
            {boolish(safety, "preflight_ready") ? "ready" : "blocked"}
          </Badge>
        </div>
        <div className="obs-kv mono">
          <span>Incident</span>
          <strong>{stringValue(incident.incident_kind, "clear")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Bundle</span>
          <strong>{stringValue(bundle.status, "none")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>End guard</span>
          <strong>{stringValue(streamEndGuard.confirmation_text, "armed")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Rec guard</span>
          <strong>{stringValue(recordingGuard.confirmation_text, "armed")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Force guard</span>
          <strong>{stringValue(forceGuard.confirmation_text, "armed")}</strong>
        </div>
        <div className="obs-safety__actions">
          <Button variant="danger" size="sm" icon={<ShieldAlert />} onClick={onEmergencyDisconnect}>Hold</Button>
          <Button variant="secondary" size="sm" onClick={() => onLiveOpsOverride("safe_mode")}>Safe</Button>
          <Button
            variant="danger"
            size="sm"
            onClick={() => {
              if (armedAction === "force_end") {
                setArmedAction(null);
                onLiveOpsOverride("force_end");
              } else {
                setArmedAction("force_end");
              }
            }}
          >
            {armedAction === "force_end" ? "FORCE END" : "Force"}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => onLiveOpsOverride("clear_incidents")}>Clear</Button>
          <Button variant="secondary" size="sm" icon={<PackageCheck />} onClick={onSupportBundle}>Bundle</Button>
        </div>
      </div>
    </Panel>
  );
}

export function GuestsPanel({
  guests,
  targetSceneId,
  onInvite,
  onRouting,
  onRelay,
  onDeviceCheck,
  onMediaTelemetry,
  onWebrtcOffer,
  onReturnFeed,
  onIsolatedRecording,
  onModerate,
  onPatch,
  onRemove,
}: {
  readonly guests: ObsRow;
  readonly targetSceneId: string;
  readonly onInvite: () => void;
  readonly onRouting: (mode: "dual" | "group" | "shared_game") => void;
  readonly onRelay: () => void;
  readonly onDeviceCheck: (participantId: string) => void;
  readonly onMediaTelemetry: (participantId: string) => void;
  readonly onWebrtcOffer: (participantId: string) => void;
  readonly onReturnFeed: (participantId: string) => void;
  readonly onIsolatedRecording: (participantId: string, recording: boolean) => void;
  readonly onModerate: (participantId: string, action: "hold_backstage" | "release_backstage" | "approve_live") => void;
  readonly onPatch: (participantId: string, patch: Record<string, unknown>) => void;
  readonly onRemove: (participantId: string) => void;
}) {
  const participants = jsonArray(guests, "participants_json");
  const routing = objectValue(guests.routing_policy_json);
  const shared = objectValue(guests.shared_program_context_json);
  const transport = objectValue(shared.media_transport);
  const degradation = objectValue(transport.degradation);
  const activeSpeaker = objectValue(shared.active_speaker);
  const roomMode = text(guests, "room_mode", "solo");
  return (
    <Panel
      title="Guests"
      icon={<Users />}
      summary={<><strong>{participants.length}/{num(guests, "max_participants")}</strong><span>{stringValue(activeSpeaker.display_name, roomMode)}</span><span>{stringValue(transport.transport, "routing")}</span></>}
      defaultCollapsed
    >
      <div className="obs-guests">
        <div className="obs-kv mono">
          <span>{roomMode}</span>
          <strong>
            {participants.length}/{num(guests, "max_participants")}
          </strong>
        </div>
        <div className="obs-kv mono">
          <span>{stringValue(routing.transport, "routing")}</span>
          <Badge tone={boolValue(routing.mix_minus) ? "hd" : "premium"}>
            {boolValue(routing.mirrored_channels) ? "mirror" : boolValue(routing.mix_minus) ? "mix-minus" : "main"}
          </Badge>
        </div>
        <div className="obs-kv mono">
          <span>{numberValue(transport.forwarded_stream_count)} streams</span>
          <Badge tone={stringValue(degradation.weak_guest_policy, "") ? "hd" : "neutral"}>
            {stringValue(degradation.weak_guest_policy, "sfu plan")}
          </Badge>
        </div>
        <div className="obs-guest__actions">
          <Button size="sm" variant={roomMode === "dual" ? "secondary" : "ghost"} onClick={() => onRouting("dual")}>
            Dual
          </Button>
          <Button size="sm" variant={roomMode === "group" ? "secondary" : "ghost"} onClick={() => onRouting("group")}>
            Group
          </Button>
          <Button size="sm" variant={roomMode === "shared_game" ? "secondary" : "ghost"} onClick={() => onRouting("shared_game")}>
            Share
          </Button>
          <Button size="sm" variant="ghost" onClick={onRelay}>
            Relay
          </Button>
        </div>
        <Button variant="secondary" size="sm" icon={<UserPlus />} onClick={onInvite} full>
          Invite
        </Button>
        {participants.map((participant) => {
          const health = objectValue(participant.connection_health_json);
          const feed = objectValue(participant.return_feed_json);
          const plan = objectValue(feed.transport_plan);
          const videoPlan = objectValue(plan.video);
          const sync = objectValue(feed.sync);
          const media = objectValue(participant.media_state_json);
          const webrtc = objectValue(participant.webrtc_session_json);
          const webrtcHealth = objectValue(webrtc.health_json);
          const relay = objectValue(participant.media_relay_json);
          const relayHealth = objectValue(relay.health_json);
          const isolated = objectValue(participant.isolated_recording_json);
          const muted = boolish(participant, "muted");
          const safetyDisabled = boolish(participant, "safety_disabled");
          const held = text(participant, "status") === "held";
          const activeSpeaker = boolValue(media.active_speaker);
          return (
            <div className="obs-guest" key={participant.id}>
              <span>
                <strong>{text(participant, "display_name")}</strong>
                <em className="mono">
                  {text(participant, "status")} / {stringValue(feed.audio, "return")} / {stringValue(feed.video, "video")}
                </em>
              </span>
              <Badge tone={activeSpeaker ? "live" : text(participant, "status") === "live" ? "hd" : safetyDisabled ? "premium" : "neutral"}>
                {activeSpeaker ? "speaking" : stringValue(health.status, text(participant, "status"))}
              </Badge>
              <em className="mono">
                {numberValue(media.audio_level_db).toFixed(0)} dB / {stringValue(sync.status, stringValue(feed.status, "return"))} / {stringValue(relayHealth.status, stringValue(webrtcHealth.status, "webrtc"))} / {stringValue(videoPlan.participant_layer, "720p30")} / iso {stringValue(isolated.status, "pending")}
              </em>
              <div className="obs-guest__actions">
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onDeviceCheck(participant.id)}
                  disabled={text(participant, "status") === "removed"}
                >
                  Check
                </Button>
                <Button
                  size="sm"
                  variant={activeSpeaker ? "secondary" : "ghost"}
                  onClick={() => onMediaTelemetry(participant.id)}
                  disabled={text(participant, "status") === "removed"}
                >
                  Speak
                </Button>
                <Button
                  size="sm"
                  variant={stringValue(webrtc.status, "") === "connected" ? "secondary" : "ghost"}
                  onClick={() => onWebrtcOffer(participant.id)}
                  disabled={text(participant, "status") === "removed"}
                >
                  WebRTC
                </Button>
                <Button
                  size="sm"
                  variant={stringValue(feed.status, "") === "ready" ? "secondary" : "ghost"}
                  onClick={() => onReturnFeed(participant.id)}
                  disabled={text(participant, "status") === "removed"}
                >
                  Return
                </Button>
                <Button
                  size="sm"
                  variant={stringValue(isolated.status, "") === "recording" ? "secondary" : "ghost"}
                  onClick={() => onIsolatedRecording(participant.id, stringValue(isolated.status, "") === "recording")}
                  disabled={text(participant, "status") === "removed"}
                >
                  Iso
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onModerate(participant.id, "approve_live")}
                  disabled={!targetSceneId || text(participant, "status") === "removed"}
                >
                  Live
                </Button>
                <Button
                  size="sm"
                  variant={held ? "secondary" : "ghost"}
                  onClick={() => onModerate(participant.id, held ? "release_backstage" : "hold_backstage")}
                  disabled={text(participant, "status") === "removed"}
                >
                  {held ? "Release" : "Hold"}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onPatch(participant.id, { muted: !muted })}
                  disabled={text(participant, "status") === "removed"}
                >
                  {muted ? "Unmute" : "Mute"}
                </Button>
                <Button
                  size="sm"
                  variant={safetyDisabled ? "danger" : "ghost"}
                  onClick={() => onPatch(participant.id, { safety_disabled: !safetyDisabled })}
                  disabled={text(participant, "status") === "removed"}
                >
                  Safe
                </Button>
                <Button size="sm" variant="ghost" onClick={() => onRemove(participant.id)}>
                  Out
                </Button>
              </div>
            </div>
          );
        })}
      </div>
    </Panel>
  );
}

export function HotkeysPanel({
  hotkeys,
  onTrigger,
  onToggle,
}: {
  readonly hotkeys: readonly ObsRow[];
  readonly onTrigger: (hotkeyId: string) => void;
  readonly onToggle: (hotkeyId: string, enabled: boolean) => void;
}) {
  return (
    <Panel
      title="Hotkeys"
      icon={<Keyboard />}
      summary={<strong>{hotkeys.filter((hotkey) => boolish(hotkey, "enabled")).length}/{hotkeys.length} on</strong>}
      defaultCollapsed
    >
      <div className="obs-hotkeys">
        {hotkeys.slice(0, 6).map((hotkey) => {
          const enabled = boolish(hotkey, "enabled");
          return (
            <div className="obs-hotkey" key={hotkey.id}>
              <span>
                <strong>{text(hotkey, "binding")}</strong>
                <em className="mono">{text(hotkey, "action")}</em>
              </span>
              <Badge tone={enabled ? "hd" : "neutral"}>{enabled ? "on" : "off"}</Badge>
              <Button size="sm" variant="ghost" onClick={() => onTrigger(hotkey.id)} disabled={!enabled}>
                Run
              </Button>
              <Button size="sm" variant="ghost" onClick={() => onToggle(hotkey.id, !enabled)}>
                {enabled ? "Off" : "On"}
              </Button>
            </div>
          );
        })}
      </div>
    </Panel>
  );
}

export function CuePanel({
  cues,
  onTrigger,
}: {
  readonly cues: readonly ObsRow[];
  readonly onTrigger: (id: string) => void;
}) {
  return (
    <Panel
      title="Sponsor Cues"
      icon={<BadgeCheck />}
      summary={<strong>{cues.filter((cue) => text(cue, "status") !== "shown_live").length} ready</strong>}
      defaultCollapsed
    >
      <div className="obs-list">
        {cues.map((cue) => (
          <button className="obs-list__row" key={cue.id} onClick={() => onTrigger(cue.id)}>
            <span>
              <strong>{text(cue, "label")}</strong>
              <em className="mono">
                {formatDuration(num(cue, "scheduled_at_seconds"))} / {num(cue, "required_duration_seconds")}s
              </em>
            </span>
            <Badge tone={text(cue, "status") === "shown_live" ? "live" : "premium"}>{text(cue, "status")}</Badge>
          </button>
        ))}
      </div>
    </Panel>
  );
}

export function RuntimePanel({
  events,
  replays,
  runtime,
  health,
  postShow,
  streamState,
  onDiscardRecording,
}: {
  readonly events: readonly ObsRow[];
  readonly replays: readonly ObsRow[];
  readonly runtime: ObsRow;
  readonly health: ObsRow;
  readonly postShow: ObsRow;
  readonly streamState: RuntimeSocketState;
  readonly onDiscardRecording: () => void;
}) {
  const latestReplay = replays[0] ?? null;
  const replayClip = objectValue(latestReplay?.clip_draft_json);
  const replayManifest = objectValue(replayClip.manifest_json);
  const replaySource = objectValue(replayManifest.source);
  const replayUpload = objectValue(replayClip.upload_queue_json);
  const replayBuffer = objectValue(replayClip.buffer_json);
  const replayPressure = objectValue(replayClip.pressure_json);
  const replayMemory = objectValue(replayPressure.memory);
  const replayAsset = objectValue(replayClip.vanta_asset_json);
  const target = objectValue(runtime.runtime_target_json);
  const output = objectValue(runtime.runtime_output_json);
  const localPublish = objectValue(output.health_json).local_publish;
  const localPublishStatus = objectValue(localPublish);
  const localPublishSegments = Array.isArray(localPublishStatus.segments) ? localPublishStatus.segments.length : 0;
  const reconnectAttempts = objectValue(objectValue(output.health_json).reconnect_attempts);
  const playback = objectValue(runtime.playback_readiness_json);
  const runtimeStatus = objectValue(runtime.runtime_status_json);
  const reconnect = objectValue(runtimeStatus.reconnect);
  const streamHealth = objectValue(runtimeStatus.stream_health);
  const adaptation = objectValue(streamHealth.adaptation);
  const packaging = objectValue(runtimeStatus.packaging);
  const archive = objectValue(runtimeStatus.archive);
  const sourceValidation = objectValue(runtimeStatus.source_validation);
  const guestRelays = objectArrayValue(runtimeStatus.guest_media_relays);
  const nativeFallback = objectValue(runtimeStatus.native_fallback);
  const externalIngest = objectValue(nativeFallback.external_ingest);
  const recording = objectValue(runtime.latest_recording_json);
  const recordingPaths = objectValue(recording.output_paths_json);
  const recordingIntegrity = objectValue(recordingPaths.integrity);
  const runtimeRecording = objectValue(recordingPaths.runtime_recording);
  const recordingAsset = objectValue(recordingPaths.vanta_asset);
  const recordingAssetSegments = Array.isArray(recordingAsset.segments) ? recordingAsset.segments.length : 0;
  const participantArchives = Array.isArray(recordingPaths.participant_archives)
    ? recordingPaths.participant_archives.length
    : 0;
  const recordingStatus = stringValue(recording.status, "");
  const canDiscardRecording = ["recording", "paused", "packaging"].includes(recordingStatus);
  const [armedDiscard, setArmedDiscard] = useState(false);
  const postMetrics = objectValue(postShow.metrics_json);
  const outputPaths = objectValue(postShow.output_paths_json);
  const archiveAsset = objectValue(outputPaths.archive_asset);
  const highlightsAsset = objectValue(outputPaths.highlights_asset);
  return (
    <Panel
      title="Runtime"
      icon={<FileVideo />}
      summary={<><strong>{streamState}</strong><span>{replays.length} replay</span></>}
      defaultCollapsed
    >
      <div className="obs-runtime">
        <div className="obs-kv mono">
          <span>Updates</span>
          <Badge tone={runtimeSocketTone(streamState)}>{streamState}</Badge>
        </div>
        <div className="obs-kv mono"><span>Target</span><strong>{stringValue(target.protocol, "pending")}</strong></div>
        <div className="obs-kv mono"><span>Output</span><strong>{stringValue(output.status, "standby")}</strong></div>
        {localPublishStatus.status ? (
          <div className="obs-kv mono">
            <span>Publish</span>
            <strong>{stringValue(localPublishStatus.status, "pending")} {localPublishSegments}</strong>
          </div>
        ) : null}
        {health.last_runtime_error ? (
          <div className="obs-kv mono">
            <span>Runtime error</span>
            <strong>{stringValue(objectValue(health.last_runtime_error).reason, "clear")}</strong>
          </div>
        ) : null}
        <div className="obs-kv mono"><span>Playback</span><strong>{stringValue(playback.status, "pending")}</strong></div>
        <div className="obs-kv mono">
          <span>Reconnect</span>
          <strong>{stringValue(reconnect.status, "armed")} {numberValue(reconnect.count)}</strong>
        </div>
        {reconnectAttempts.status ? (
          <div className="obs-kv mono">
            <span>Retry</span>
            <strong>{stringValue(reconnectAttempts.status, "armed")} {numberValue(reconnectAttempts.next_backoff_ms)}ms</strong>
          </div>
        ) : null}
        <div className="obs-kv mono">
          <span>Stream</span>
          <strong>{stringValue(streamHealth.status, text(health, "status"))} / {stringValue(streamHealth.dynamic_bitrate, text(health, "dynamic_bitrate", "stable"))}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Native</span>
          <strong>{fallbackLabel(nativeFallback.status)}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Ingest fallback</span>
          <strong>{boolValue(externalIngest.available) ? "ready" : "pending"}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Adapt</span>
          <strong>{numberValue(adaptation.target_bitrate_kbps)} kbps {stringValue(adaptation.target_resolution, "")}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Sources</span>
          <strong>
            {stringValue(sourceValidation.status, "pending")} {numberValue(sourceValidation.ready)}/{numberValue(sourceValidation.total)}
          </strong>
        </div>
        <div className="obs-kv mono">
          <span>Guest relays</span>
          <strong>{guestRelays.filter((relay) => stringValue(relay.status, "") === "relaying").length}/{guestRelays.length}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Packaging</span>
          <strong>{stringValue(packaging.status, text(postShow, "status"))}</strong>
        </div>
        <div className="obs-kv mono">
          <span>Archive</span>
          <strong>{stringValue(archive.status, "pending")}</strong>
        </div>
        {recording.id ? (
          <div className="obs-kv mono">
            <span>{stringValue(recording.status, "recording")}</span>
            <strong>
              {stringValue(recordingIntegrity.status, "pending")} {numberValue(recordingIntegrity.segments_verified)}
            </strong>
          </div>
        ) : null}
        {runtimeRecording.status ? (
          <div className="obs-kv mono">
            <span>Long session</span>
            <strong>
              {stringValue(runtimeRecording.status, "armed")} {numberValue(runtimeRecording.logical_chunk_count || 1)}
            </strong>
          </div>
        ) : null}
        {recordingAsset.status ? (
          <div className="obs-kv mono">
            <span>Recording asset</span>
            <strong>
              {stringValue(recordingAsset.status, "pending")} {recordingAssetSegments}
            </strong>
          </div>
        ) : null}
        {participantArchives > 0 ? (
          <div className="obs-kv mono">
            <span>Participants</span>
            <strong>{participantArchives} archived</strong>
          </div>
        ) : null}
        {canDiscardRecording ? (
          <Button
            size="sm"
            variant={armedDiscard ? "danger" : "secondary"}
            onClick={() => {
              if (armedDiscard) {
                setArmedDiscard(false);
                onDiscardRecording();
              } else {
                setArmedDiscard(true);
              }
            }}
          >
            {armedDiscard ? "DISCARD RECORDING" : "Discard"}
          </Button>
        ) : null}
        <div className="obs-kv mono"><span>Replays</span><strong>{replays.length}</strong></div>
        {latestReplay ? (
          <div className="obs-kv mono">
            <span>{text(latestReplay, "status")}</span>
            <strong>{stringValue(replayUpload.status, stringValue(replayManifest.kind, "queued"))}</strong>
          </div>
        ) : null}
        {replayBuffer.status ? (
          <div className="obs-kv mono">
            <span>Buffer</span>
            <strong>
              {numberValue(replayBuffer.selected_segment_count)} chunks / {stringValue(replayPressure.disk_pressure, "ok")}
            </strong>
          </div>
        ) : null}
        {replaySource.kind ? (
          <div className="obs-kv mono">
            <span>Replay source</span>
            <strong>{stringValue(replaySource.kind, "fallback")}</strong>
          </div>
        ) : null}
        {replayAsset.status ? (
          <div className="obs-kv mono">
            <span>Vanta asset</span>
            <strong>{stringValue(replayUpload.status, "queued")} / {stringValue(replayMemory.status, "ok")}</strong>
          </div>
        ) : null}
        <div className="obs-kv mono"><span>Post show</span><strong>{text(postShow, "status")}</strong></div>
        {postShow.metrics_json ? (
          <div className="obs-kv mono">
            <span>{stringValue(postMetrics.archive_integrity, "archive")}</span>
            <strong>{numberValue(postMetrics.clip_pack_count)} clips / {numberValue(postMetrics.proof_count)} proofs</strong>
          </div>
        ) : null}
        {archiveAsset.status ? (
          <div className="obs-kv mono">
            <span>Archive asset</span>
            <strong>{stringValue(archiveAsset.status, "pending")} / {stringValue(highlightsAsset.status, "pending")}</strong>
          </div>
        ) : null}
        {events.slice(0, 4).map((event) => (
          <div className="obs-event" key={event.id}>
            <Scissors size={13} />
            <span>{text(event, "message")}</span>
          </div>
        ))}
      </div>
    </Panel>
  );
}
