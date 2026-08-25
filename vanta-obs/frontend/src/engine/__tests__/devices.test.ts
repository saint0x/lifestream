import { describe, expect, it, vi } from "vitest";
import {
  EMPTY_SESSIONS,
  enumerateCaptureDevices,
  reconcileCatalogSessions,
  readySession,
  requestCaptureStream,
  stopCaptureSession,
  type MediaDevicesPort,
} from "../devices";

describe("device acquisition engine", () => {
  it("enumerates camera, microphone, and display support", async () => {
    const mediaDevices: MediaDevicesPort = {
      enumerateDevices: async () => [
        device("cam-a", "videoinput", "Sony FX3"),
        device("mic-a", "audioinput", "Host Lav"),
        device("speaker", "audiooutput", "Monitor"),
      ],
      getDisplayMedia: vi.fn(),
    };

    const catalog = await enumerateCaptureDevices(mediaDevices);

    expect(catalog.cameras).toEqual([{ id: "cam-a", kind: "camera", label: "Sony FX3" }]);
    expect(catalog.microphones).toEqual([{ id: "mic-a", kind: "microphone", label: "Host Lav" }]);
    expect(catalog.displaySupported).toBe(true);
  });

  it("requests the correct browser constraints for each capture kind", async () => {
    const stream = mediaStream("video");
    const getUserMedia = vi.fn(async () => stream);
    const getDisplayMedia = vi.fn(async () => stream);
    const mediaDevices: MediaDevicesPort = { getUserMedia, getDisplayMedia };

    await requestCaptureStream("camera", mediaDevices);
    await requestCaptureStream("microphone", mediaDevices);
    await requestCaptureStream("display", mediaDevices);

    expect(getUserMedia).toHaveBeenNthCalledWith(1, { video: true, audio: false });
    expect(getUserMedia).toHaveBeenNthCalledWith(2, { video: false, audio: true });
    expect(getDisplayMedia).toHaveBeenCalledWith({ video: true, audio: true });
  });

  it("stops live media tracks when a session is disarmed", () => {
    const stop = vi.fn();
    const session = readySession("camera", mediaStream("video", stop));

    const next = stopCaptureSession(session);

    expect(stop).toHaveBeenCalledTimes(1);
    expect(next.status).toBe("idle");
    expect(next.stream).toBeNull();
  });

  it("preserves the selected device id and reconnects with an exact constraint", async () => {
    const stream = mediaStream("video", vi.fn(), "cam-a", "Sony FX3");
    const getUserMedia = vi.fn(async () => stream);
    const mediaDevices: MediaDevicesPort = { getUserMedia };

    const session = await requestCaptureStream("camera", mediaDevices, "cam-a", 2);

    expect(getUserMedia).toHaveBeenCalledWith({
      video: { deviceId: { exact: "cam-a" } },
      audio: false,
    });
    expect(session.status).toBe("ready");
    expect(session.deviceId).toBe("cam-a");
    expect(session.label).toBe("Sony FX3");
    expect(session.reconnectAttempts).toBe(2);
  });

  it("marks missing hotplugged devices for reconnect and waits when reacquire fails", async () => {
    const stop = vi.fn();
    const session = readySession("camera", mediaStream("video", stop, "cam-a", "Sony FX3"));

    const missing = reconcileCatalogSessions(
      { cameras: [], microphones: [], displaySupported: true },
      { ...EMPTY_SESSIONS, camera: session },
    );

    expect(stop).toHaveBeenCalledTimes(1);
    expect(missing.camera.status).toBe("reconnecting");
    expect(missing.camera.deviceId).toBe("cam-a");

    const getUserMedia = vi.fn(async () => {
      throw new DOMException("Device no longer available", "OverconstrainedError");
    });
    const disconnected = await requestCaptureStream("camera", { getUserMedia }, "cam-a", 1);

    expect(disconnected.status).toBe("disconnected");
    expect(disconnected.deviceId).toBe("cam-a");
    expect(disconnected.reconnectAttempts).toBe(1);
  });

  it("marks a returned disconnected device for reconnect on the next devicechange", () => {
    const disconnected = {
      ...readySession("microphone", mediaStream("audio", vi.fn(), "mic-a", "Host Lav")),
      status: "disconnected" as const,
      stream: null,
      tracks: [],
      error: "Host Lav media track ended.",
    };

    const reconciled = reconcileCatalogSessions(
      {
        cameras: [],
        microphones: [{ id: "mic-a", kind: "microphone", label: "Host Lav" }],
        displaySupported: true,
      },
      { ...EMPTY_SESSIONS, microphone: disconnected },
    );

    expect(reconciled.microphone.status).toBe("reconnecting");
    expect(reconciled.microphone.reconnectAttempts).toBe(1);
  });

  it("reports denied permission distinctly from unsupported capture", async () => {
    const deniedDevices: MediaDevicesPort = {
      getUserMedia: async () => {
        throw new DOMException("Permission denied", "NotAllowedError");
      },
    };

    const denied = await requestCaptureStream("camera", deniedDevices);
    const unsupported = await requestCaptureStream("display", {});

    expect(denied.status).toBe("denied");
    expect(unsupported.status).toBe("error");
    expect(unsupported.error).toContain("Display capture");
  });
});

function device(deviceId: string, kind: MediaDeviceKind, label: string): MediaDeviceInfo {
  return {
    deviceId,
    groupId: "group",
    kind,
    label,
    toJSON: () => ({}),
  };
}

function mediaStream(
  kind: "audio" | "video",
  stop = vi.fn(),
  deviceId = `${kind}-device`,
  label = `${kind} device`,
): MediaStream {
  return {
    getTracks: () => [
      {
        kind,
        label,
        readyState: "live",
        stop,
        getSettings: () => ({ deviceId }),
      } as unknown as MediaStreamTrack,
    ],
  } as MediaStream;
}
