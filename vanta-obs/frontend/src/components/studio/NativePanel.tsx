import { AlertTriangle, Cpu, HeartPulse, RotateCcw, Square } from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import type { NativeHelperPackage, NativeHelperSession, ObsRow } from "@/types";
import { boolish, num, text } from "@/types";
import { Panel } from "./Panel";

export function NativePanel({
  sessions,
  packages,
  busy,
  onStart,
  onHeartbeat,
  onRecover,
  onCrashReport,
  onShutdown,
}: {
  readonly sessions: readonly NativeHelperSession[];
  readonly packages: readonly NativeHelperPackage[];
  readonly busy: boolean;
  readonly onStart: (kind: string) => void;
  readonly onHeartbeat: (sessionId: string) => void;
  readonly onRecover: (sessionId: string) => void;
  readonly onCrashReport: (sessionId: string) => void;
  readonly onShutdown: (sessionId: string) => void;
}) {
  const ready = sessions.filter((session) => text(session, "status") === "ready").length;
  const crashes = sessions.reduce((total, session) => total + num(session, "crash_count"), 0);
  const readyPackages = packages.filter((pkg) => text(pkg, "status") === "ready").length;
  return (
    <Panel
      title="Native"
      icon={<Cpu />}
      summary={<><strong>{ready}/{sessions.length} ready</strong><span>{readyPackages}/{packages.length} pkg</span><span>{crashes} crash</span></>}
      defaultCollapsed
    >
      <div className="obs-native">
        <div className="obs-native__actions">
          <Button size="sm" variant="secondary" icon={<Cpu />} onClick={() => onStart("capture")} disabled={busy}>
            Capture
          </Button>
          <Button size="sm" variant="secondary" icon={<Cpu />} onClick={() => onStart("encode")} disabled={busy}>
            Encode
          </Button>
        </div>
        {packages.slice(0, 4).map((pkg) => (
          <div className="obs-native__package" key={text(pkg, "package_id")}>
            <span>
              <strong>{text(pkg, "helper_kind")}</strong>
              <em className="mono">{text(pkg, "platform")} / {packageMeta(pkg)}</em>
            </span>
            <Badge tone={packageTone(pkg)}>{text(pkg, "status")}</Badge>
          </div>
        ))}
        {sessions.slice(0, 3).map((session) => (
          <div className="obs-native__session" key={session.id}>
            <span>
              <strong>{text(session, "helper_kind")}</strong>
              <em className="mono">{nativeMeta(session)}</em>
            </span>
            <Badge tone={nativeTone(session)}>{text(session, "status")}</Badge>
            <Button size="sm" variant="ghost" icon={<HeartPulse />} onClick={() => onHeartbeat(session.id)} disabled={busy} />
            <Button size="sm" variant="ghost" icon={<RotateCcw />} onClick={() => onRecover(session.id)} disabled={busy} />
            <Button size="sm" variant="ghost" icon={<AlertTriangle />} onClick={() => onCrashReport(session.id)} disabled={busy} />
            <Button size="sm" variant="ghost" icon={<Square />} onClick={() => onShutdown(session.id)} disabled={busy} />
          </div>
        ))}
      </div>
    </Panel>
  );
}

function packageMeta(pkg: NativeHelperPackage): string {
  const diagnostics = Array.isArray(pkg.diagnostics) ? pkg.diagnostics.length : 0;
  return diagnostics ? `${diagnostics} check` : text(pkg, "binary_path");
}

function packageTone(pkg: NativeHelperPackage): "hd" | "premium" | "neutral" {
  const status = text(pkg, "status");
  if (status === "ready") return "hd";
  if (status === "available_for_other_platform") return "neutral";
  return "premium";
}

function nativeMeta(session: NativeHelperSession): string {
  const health = objectValue(session.health_json);
  const recovered = boolish(health, "recovered") ? "recovered" : text(session, "endpoint");
  const trace = text(health, "trace_event");
  const crashes = num(session, "crash_count");
  return [recovered, crashes ? `${crashes} crash` : "", trace].filter(Boolean).join(" / ");
}

function nativeTone(session: NativeHelperSession): "hd" | "premium" | "neutral" {
  const status = text(session, "status");
  if (status === "ready") return "hd";
  if (status === "stopped") return "neutral";
  return "premium";
}

function objectValue(value: unknown): ObsRow {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as ObsRow
    : { id: "" };
}
