export type CaptureKind = "camera" | "microphone" | "display";
export type CaptureStatus =
  | "idle"
  | "checking"
  | "ready"
  | "reconnecting"
  | "disconnected"
  | "denied"
  | "unsupported"
  | "error";

export interface CaptureDevice {
  readonly id: string;
  readonly label: string;
  readonly kind: CaptureKind;
}

export interface DeviceCatalog {
  readonly cameras: readonly CaptureDevice[];
  readonly microphones: readonly CaptureDevice[];
  readonly displaySupported: boolean;
}

export interface CaptureSession {
  readonly kind: CaptureKind;
  readonly status: CaptureStatus;
  readonly label: string;
  readonly stream: MediaStream | null;
  readonly tracks: readonly string[];
  readonly deviceId: string | null;
  readonly reconnectAttempts: number;
  readonly error: string | null;
}

export interface MediaDevicesPort {
  readonly enumerateDevices?: () => Promise<readonly MediaDeviceInfo[]>;
  readonly getUserMedia?: (constraints: MediaStreamConstraints) => Promise<MediaStream>;
  readonly getDisplayMedia?: (constraints: DisplayMediaStreamOptions) => Promise<MediaStream>;
  readonly addEventListener?: (type: "devicechange", listener: () => void) => void;
  readonly removeEventListener?: (type: "devicechange", listener: () => void) => void;
}

export const EMPTY_CATALOG: DeviceCatalog = {
  cameras: [],
  microphones: [],
  displaySupported: false,
};

export const EMPTY_SESSIONS: Record<CaptureKind, CaptureSession> = {
  camera: idleSession("camera", "Camera"),
  microphone: idleSession("microphone", "Microphone"),
  display: idleSession("display", "Display"),
};

export function browserMediaDevices(): MediaDevicesPort | null {
  return typeof navigator === "undefined" ? null : navigator.mediaDevices ?? null;
}

export async function enumerateCaptureDevices(
  mediaDevices: MediaDevicesPort | null = browserMediaDevices(),
): Promise<DeviceCatalog> {
  if (!mediaDevices?.enumerateDevices) return EMPTY_CATALOG;
  const devices = await mediaDevices.enumerateDevices();
  return {
    cameras: devices
      .filter((device) => device.kind === "videoinput")
      .map((device, index) => captureDevice(device, "camera", `Camera ${index + 1}`)),
    microphones: devices
      .filter((device) => device.kind === "audioinput")
      .map((device, index) => captureDevice(device, "microphone", `Microphone ${index + 1}`)),
    displaySupported: typeof mediaDevices.getDisplayMedia === "function",
  };
}

export async function requestCaptureStream(
  kind: CaptureKind,
  mediaDevices: MediaDevicesPort | null = browserMediaDevices(),
  preferredDeviceId?: string | null,
  reconnectAttempts = 0,
): Promise<CaptureSession> {
  if (!mediaDevices) return unsupportedSession(kind);
  try {
    const stream = await acquire(kind, mediaDevices, preferredDeviceId);
    return readySession(kind, stream, reconnectAttempts);
  } catch (error) {
    return failedSession(kind, error, reconnectAttempts, preferredDeviceId ?? null);
  }
}

export function stopCaptureSession(session: CaptureSession): CaptureSession {
  session.stream?.getTracks().forEach((track) => track.stop());
  return idleSession(session.kind, session.label);
}

export function readySession(kind: CaptureKind, stream: MediaStream, reconnectAttempts = 0): CaptureSession {
  const tracks = stream.getTracks();
  const primary = primaryTrack(kind, tracks);
  return {
    kind,
    status: "ready",
    label: primary?.label || labelFor(kind),
    stream,
    tracks: tracks.map((track) => `${track.kind}:${track.readyState}`),
    deviceId: trackDeviceId(primary),
    reconnectAttempts,
    error: null,
  };
}

export function idleSession(kind: CaptureKind, label = labelFor(kind)): CaptureSession {
  return {
    kind,
    status: "idle",
    label,
    stream: null,
    tracks: [],
    deviceId: null,
    reconnectAttempts: 0,
    error: null,
  };
}

export function reconnectingSession(session: CaptureSession, reason: string): CaptureSession {
  session.stream?.getTracks().forEach((track) => track.stop());
  return {
    ...session,
    status: "reconnecting",
    tracks: [],
    stream: null,
    reconnectAttempts: session.reconnectAttempts + 1,
    error: reason,
  };
}

export function disconnectedSession(session: CaptureSession, reason: string): CaptureSession {
  session.stream?.getTracks().forEach((track) => track.stop());
  return {
    ...session,
    status: "disconnected",
    stream: null,
    tracks: [],
    error: reason,
  };
}

export function reconcileCatalogSessions(
  catalog: DeviceCatalog,
  sessions: Record<CaptureKind, CaptureSession>,
): Record<CaptureKind, CaptureSession> {
  return {
    camera: reconcileCatalogSession(sessions.camera, catalog.cameras),
    microphone: reconcileCatalogSession(sessions.microphone, catalog.microphones),
    display: sessions.display,
  };
}

export function shouldReconnectSession(session: CaptureSession): boolean {
  return session.status === "reconnecting" && session.kind !== "display";
}

function unsupportedSession(kind: CaptureKind): CaptureSession {
  return {
    ...idleSession(kind),
    status: "unsupported",
    error: "Browser media capture is not available in this runtime.",
  };
}

function failedSession(
  kind: CaptureKind,
  error: unknown,
  reconnectAttempts: number,
  deviceId: string | null,
): CaptureSession {
  const name = error instanceof DOMException ? error.name : "";
  const reconnectFailed = reconnectAttempts > 0 && deviceId;
  return {
    ...idleSession(kind),
    status: reconnectFailed
      ? "disconnected"
      : name === "NotAllowedError" || name === "SecurityError" ? "denied" : "error",
    deviceId,
    reconnectAttempts,
    error: error instanceof Error ? error.message : "Capture request failed.",
  };
}

async function acquire(
  kind: CaptureKind,
  mediaDevices: MediaDevicesPort,
  preferredDeviceId?: string | null,
): Promise<MediaStream> {
  if (kind === "camera") {
    if (!mediaDevices.getUserMedia) throw new Error("Camera capture is unavailable.");
    return mediaDevices.getUserMedia({ video: deviceConstraint(preferredDeviceId), audio: false });
  }
  if (kind === "microphone") {
    if (!mediaDevices.getUserMedia) throw new Error("Microphone capture is unavailable.");
    return mediaDevices.getUserMedia({ video: false, audio: deviceConstraint(preferredDeviceId) });
  }
  if (!mediaDevices.getDisplayMedia) throw new Error("Display capture is unavailable.");
  return mediaDevices.getDisplayMedia({ video: true, audio: true });
}

function captureDevice(device: MediaDeviceInfo, kind: CaptureKind, fallback: string): CaptureDevice {
  return {
    id: device.deviceId,
    label: device.label || fallback,
    kind,
  };
}

function labelFor(kind: CaptureKind): string {
  if (kind === "camera") return "Camera";
  if (kind === "microphone") return "Microphone";
  return "Display";
}

function reconcileCatalogSession(
  session: CaptureSession,
  devices: readonly CaptureDevice[],
): CaptureSession {
  if (!session.deviceId) return session;
  const devicePresent = devices.some((device) => device.id === session.deviceId);
  if (session.status === "ready") {
    return devicePresent
      ? session
      : reconnectingSession(session, `${session.label} disconnected; waiting for the device to return.`);
  }
  if (session.status === "disconnected") {
    return devicePresent
      ? reconnectingSession(session, `${session.label} returned; reconnecting.`)
      : session;
  }
  return session;
}

function deviceConstraint(deviceId?: string | null): boolean | MediaTrackConstraints {
  return deviceId ? { deviceId: { exact: deviceId } } : true;
}

function primaryTrack(kind: CaptureKind, tracks: readonly MediaStreamTrack[]): MediaStreamTrack | undefined {
  const expectedKind = kind === "microphone" ? "audio" : "video";
  return tracks.find((track) => track.kind === expectedKind) ?? tracks[0];
}

function trackDeviceId(track: MediaStreamTrack | undefined): string | null {
  if (!track?.getSettings) return null;
  const deviceId = track.getSettings().deviceId;
  return typeof deviceId === "string" && deviceId.trim() ? deviceId : null;
}
