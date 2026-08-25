import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MediaPanel } from "../MediaPanel";
import type { MediaCaptureInventory, ObsRow } from "@/types";

describe("studio MediaPanel", () => {
  it("surfaces native camera and microphone permission state compactly", () => {
    const markup = renderToStaticMarkup(
      <MediaPanel
        selectedSource={row("source_camera", { source_kind: "camera" })}
        capabilities={null}
        inventory={inventory()}
        captureSessions={[{
          id: "capture_session",
          source_id: "source_camera",
          capture_kind: "camera",
          status: "capturing",
        }]}
        captureFrames={[]}
        captureArtifacts={[]}
        sourceArtifacts={[]}
        encodeJobs={[]}
        packages={[]}
        busy={false}
        onStartCapture={() => undefined}
        onStopCapture={() => undefined}
        onCaptureReconcile={() => undefined}
        onCapturePreviewFrame={() => undefined}
        onCaptureSegment={() => undefined}
        onSourceAudioIngest={() => undefined}
        onStartEncode={() => undefined}
        onStopEncode={() => undefined}
        onRenderEncode={() => undefined}
        onPackageEncode={() => undefined}
      />,
    );

    expect(markup).toContain("Cam prompt required");
    expect(markup).toContain("Mic denied");
    expect(markup).toContain("ffmpeg_avfoundation");
    expect(markup).toContain("Reconnect capture session");
  });
});

function inventory(): MediaCaptureInventory {
  return {
    id: "inventory",
    platform: "macos",
    transport: "ffmpeg_avfoundation",
    status: "ready",
    support: {
      camera: true,
      microphone: true,
      desktop_audio: false,
      system_audio: true,
      display: true,
      window: false,
      application_audio: false,
    },
    permissions: {
      camera: { status: "prompt_required", required: true },
      microphone: { status: "denied", required: true },
    },
    devices: [{ id: "camera_0", kind: "camera", label: "FaceTime" }],
  };
}

function row(id: string, fields: Record<string, unknown> = {}): ObsRow {
  return { id, ...fields };
}
