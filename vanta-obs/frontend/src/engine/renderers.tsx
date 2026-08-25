import {
  BadgeCheck,
  Camera,
  Captions,
  Clock3,
  FileImage,
  FileVideo,
  Hash,
  MessageSquare,
  Mic2,
  MonitorUp,
  QrCode,
  Tv,
  UserRound,
  Volume2,
} from "lucide-react";
import type { SceneGraphItem } from "@/engine/graph";
import type { ObsRow } from "@/types";
import { text } from "@/types";

export interface SourceRendererModel {
  readonly kind: string;
  readonly label: string;
  readonly detail: string;
  readonly tone: string;
  readonly mediaUrl: string;
  readonly color: string;
}

export function sourceRendererModel(source: ObsRow): SourceRendererModel {
  const kind = text(source, "source_kind", "unknown");
  const settings = objectValue(source.default_settings_json);
  const label = text(source, "display_name", kind);
  const mediaUrl = firstText(settings, source, ["src", "url", "asset_url", "media_url", "image_url", "video_url", "media_path", "browser_url"]);
  const directDetail = firstText(settings, source, [
    "headline",
    "text",
    "cta_text",
    "promo_code",
    "target_url",
    "browser_url",
    "device_id",
    "media_asset_id",
  ]);

  switch (kind) {
    case "camera":
      return model(kind, label, directDetail || "camera feed", "capture", mediaUrl);
    case "microphone":
      return model(kind, label, directDetail || "program audio", "audio", mediaUrl);
    case "application_audio":
      return model(kind, label, directDetail || "application audio", "audio", mediaUrl);
    case "desktop_audio":
    case "system_audio":
      return model(kind, label, directDetail || "system mix", "audio", mediaUrl);
    case "screen_capture":
    case "display_capture":
      return model(kind, label, directDetail || "display capture", "screen", mediaUrl);
    case "window_capture":
      return model(kind, label, directDetail || "window capture", "screen", mediaUrl);
    case "browser_capture":
      return model(kind, label, directDetail || text(source, "browser_url", "browser"), "browser", mediaUrl);
    case "media_file":
    case "vanta_video_asset":
    case "vanta_clip":
      return model(kind, label, directDetail || text(source, "media_asset_id", "video asset"), "media", mediaUrl);
    case "image":
      return model(kind, label, directDetail || text(source, "media_asset_id", "image asset"), "image", mediaUrl);
    case "text":
      return model(kind, label, directDetail || "text overlay", "text", mediaUrl);
    case "lower_third":
      return model(kind, firstText(settings, source, ["headline"]) || label, firstText(settings, source, ["subhead", "subtitle"]) || label, "lower", mediaUrl);
    case "branded_bumper":
      return model(kind, firstText(settings, source, ["headline"]) || label, directDetail || "bumper", "brand", mediaUrl);
    case "pinned_cta":
      return model(kind, directDetail || label, firstText(settings, source, ["target_url"]) || "pinned CTA", "cta", mediaUrl);
    case "qr_code":
      return model(kind, label, directDetail || "scan target", "qr", mediaUrl);
    case "promo_code":
      return model(kind, directDetail || label, "promo", "promo", mediaUrl);
    case "sponsor_card":
      return model(kind, firstText(settings, source, ["brand", "sponsor_name"]) || label, directDetail || "sponsor", "sponsor", mediaUrl);
    case "countdown_timer":
      return model(kind, countdownLabel(settings), "countdown", "timer", mediaUrl);
    case "chat_overlay":
      return model(kind, label, directDetail || "live chat", "chat", mediaUrl);
    case "alert_overlay":
      return model(kind, label, directDetail || "alerts", "alert", mediaUrl);
    case "guest_feed":
      return model(kind, label, directDetail || "guest video", "guest", mediaUrl);
    case "remote_contribution":
      return model(kind, label, directDetail || "remote feed", "guest", mediaUrl);
    case "color_matte":
      return model(kind, label, directDetail || "matte", "matte", mediaUrl, firstText(settings, source, ["color"]) || "#202020");
    case "safe_area_guide":
      return model(kind, label, directDetail || "safe area", "guide", mediaUrl);
    default:
      return model(kind, label, directDetail || kind, "generic", mediaUrl);
  }
}

export function SourceRenderer({ item }: { readonly item: SceneGraphItem }) {
  const model = sourceRendererModel(item.source);
  const style = {
    left: `${item.leftPct}%`,
    top: `${item.topPct}%`,
    width: `${item.widthPct}%`,
    height: `${item.heightPct}%`,
    opacity: item.opacity,
    zIndex: item.zIndex,
  };

  return (
    <div
      className={`obs-source obs-source--${model.kind} obs-source--tone-${model.tone}`}
      style={style}
      data-source-kind={model.kind}
    >
      <SourceMedia model={model} />
      <div className="obs-source__content" style={model.tone === "matte" ? { backgroundColor: model.color } : undefined}>
        {sourceIcon(model.kind)}
        <strong>{model.label}</strong>
        <em className="mono">{model.detail}</em>
      </div>
    </div>
  );
}

function SourceMedia({ model }: { readonly model: SourceRendererModel }) {
  if (!model.mediaUrl) return null;
  if (model.tone === "browser") {
    return (
      <iframe
        className="obs-source__media"
        src={model.mediaUrl}
        title={`${model.label} browser source`}
        sandbox="allow-forms allow-presentation allow-scripts"
        referrerPolicy="no-referrer"
      />
    );
  }
  if (model.tone === "image") {
    return <img className="obs-source__media" src={model.mediaUrl} alt="" crossOrigin="anonymous" />;
  }
  if (model.tone === "media") {
    return <video className="obs-source__media" src={model.mediaUrl} muted playsInline loop crossOrigin="anonymous" />;
  }
  return null;
}

function sourceIcon(kind: string) {
  if (kind === "camera") return <Camera size={16} />;
  if (kind === "microphone") return <Mic2 size={16} />;
  if (kind === "desktop_audio" || kind === "system_audio" || kind === "application_audio") return <Volume2 size={16} />;
  if (kind.includes("screen") || kind.includes("display") || kind.includes("window") || kind.includes("browser")) return <MonitorUp size={16} />;
  if (kind === "image") return <FileImage size={16} />;
  if (kind.includes("video") || kind.includes("clip") || kind === "media_file") return <FileVideo size={16} />;
  if (kind === "text" || kind === "lower_third") return <Captions size={16} />;
  if (kind === "qr_code") return <QrCode size={16} />;
  if (kind === "promo_code") return <Hash size={16} />;
  if (kind.includes("sponsor") || kind.includes("brand") || kind.includes("cta")) return <BadgeCheck size={16} />;
  if (kind === "countdown_timer") return <Clock3 size={16} />;
  if (kind.includes("chat") || kind.includes("alert")) return <MessageSquare size={16} />;
  if (kind.includes("guest") || kind.includes("remote")) return <UserRound size={16} />;
  return <Tv size={16} />;
}

function model(kind: string, label: string, detail: string, tone: string, mediaUrl: string, color = ""): SourceRendererModel {
  return { kind, label, detail, tone, mediaUrl, color };
}

function countdownLabel(settings: Record<string, unknown>): string {
  const seconds = settings.seconds;
  return typeof seconds === "number" ? `${seconds}s` : "countdown";
}

function objectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function firstText(settings: Record<string, unknown>, row: ObsRow, keys: readonly string[]): string {
  for (const key of keys) {
    const fromSettings = settings[key];
    if (typeof fromSettings === "string" && fromSettings.trim()) return fromSettings;
    const fromRow = row[key];
    if (typeof fromRow === "string" && fromRow.trim()) return fromRow;
  }
  return "";
}
