import { useEffect, useRef } from "react";
import { Camera, Mic2, MonitorUp, Square } from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import type { CaptureKind, CaptureSession, DeviceCatalog } from "@/engine/devices";
import { Panel } from "./Panel";

export function DevicePanel({
  catalog,
  sessions,
  onRequest,
  onReconnect,
  onStop,
}: {
  readonly catalog: DeviceCatalog;
  readonly sessions: Record<CaptureKind, CaptureSession>;
  readonly onRequest: (kind: CaptureKind) => void;
  readonly onReconnect?: (kind: CaptureKind) => void;
  readonly onStop: (kind: CaptureKind) => void;
}) {
  const armed = Object.values(sessions).filter((session) => session.status === "ready").length;
  const recovering = Object.values(sessions).filter((session) => (
    session.status === "checking" || session.status === "reconnecting"
  )).length;
  return (
    <Panel
      title="Devices"
      icon={<Camera />}
      summary={<strong>{recovering > 0 ? `${recovering} reconnecting` : `${armed}/3 ready`}</strong>}
      defaultCollapsed
    >
      <div className="obs-devices">
        <DeviceRow
          kind="camera"
          label={catalog.cameras[0]?.label ?? "Camera"}
          count={catalog.cameras.length}
          session={sessions.camera}
          onRequest={onRequest}
          onReconnect={onReconnect}
          onStop={onStop}
        />
        <DeviceRow
          kind="microphone"
          label={catalog.microphones[0]?.label ?? "Microphone"}
          count={catalog.microphones.length}
          session={sessions.microphone}
          onRequest={onRequest}
          onReconnect={onReconnect}
          onStop={onStop}
        />
        <DeviceRow
          kind="display"
          label="Display"
          count={catalog.displaySupported ? 1 : 0}
          session={sessions.display}
          onRequest={onRequest}
          onReconnect={onReconnect}
          onStop={onStop}
        />
      </div>
    </Panel>
  );
}

function DeviceRow({
  kind,
  label,
  count,
  session,
  onRequest,
  onReconnect,
  onStop,
}: {
  readonly kind: CaptureKind;
  readonly label: string;
  readonly count: number;
  readonly session: CaptureSession;
  readonly onRequest: (kind: CaptureKind) => void;
  readonly onReconnect?: (kind: CaptureKind) => void;
  readonly onStop: (kind: CaptureKind) => void;
}) {
  const ready = session.status === "ready";
  const reconnectable = (session.status === "disconnected" || session.status === "error") && Boolean(session.deviceId);
  return (
    <div className="obs-device">
      <DevicePreview kind={kind} session={session} />
      <div className="obs-device__body">
        <span>
          <strong>{label}</strong>
          <em className="mono">{deviceMeta(kind, count, session)}</em>
        </span>
        <Badge tone={ready ? "hd" : session.status === "denied" || session.status === "error" ? "premium" : "neutral"}>
          {session.status}
        </Badge>
      </div>
      {ready ? (
        <Button size="sm" variant="secondary" icon={<Square />} onClick={() => onStop(kind)}>
          Stop
        </Button>
      ) : reconnectable && onReconnect ? (
        <Button size="sm" variant="secondary" icon={deviceIcon(kind)} onClick={() => onReconnect(kind)}>
          Reconnect
        </Button>
      ) : (
        <Button size="sm" variant="secondary" icon={deviceIcon(kind)} onClick={() => onRequest(kind)}>
          Arm
        </Button>
      )}
    </div>
  );
}

function DevicePreview({ kind, session }: { readonly kind: CaptureKind; readonly session: CaptureSession }) {
  const videoRef = useRef<HTMLVideoElement | null>(null);

  useEffect(() => {
    if (!videoRef.current) return;
    videoRef.current.srcObject = session.stream;
  }, [session.stream]);

  if (kind === "microphone") {
    return <div className="obs-device__icon">{deviceIcon(kind)}</div>;
  }

  return (
    <div className="obs-device__preview">
      {session.stream ? <video ref={videoRef} autoPlay muted playsInline /> : deviceIcon(kind)}
    </div>
  );
}

function deviceMeta(kind: CaptureKind, count: number, session: CaptureSession): string {
  if (session.error) return session.error;
  if (session.tracks.length > 0) return session.tracks.join(" / ");
  if (session.status === "reconnecting" || session.status === "checking") return "reconnecting";
  if (session.status === "disconnected") return "waiting for device";
  if (kind === "display") return count > 0 ? "screen share available" : "unsupported";
  return `${count} available`;
}

function deviceIcon(kind: CaptureKind) {
  if (kind === "camera") return <Camera size={15} />;
  if (kind === "microphone") return <Mic2 size={15} />;
  return <MonitorUp size={15} />;
}
