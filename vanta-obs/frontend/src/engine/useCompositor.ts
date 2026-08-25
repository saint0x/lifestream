import { useEffect, useMemo, useRef, useState } from "react";
import {
  canvasDrawableMediaUrl,
  compositorBackendLabel,
  createGpuSceneRenderer,
  type SourceStreams,
  type SourceMediaElement,
  type CompositorBackend,
  drawScene,
  renderRects,
  type RenderRect,
  type GpuSceneRenderer,
} from "./compositor";
import type { CaptureKind } from "./devices";
import type { SceneGraphItem } from "./graph";

export interface CanvasCaptureState {
  readonly supported: boolean;
  readonly active: boolean;
  readonly tracks: readonly string[];
  readonly stream: MediaStream | null;
  readonly backend: CompositorBackend;
  readonly label: string;
}

export function useCompositorCanvas({
  items,
  streams,
  width = 1920,
  height = 1080,
}: {
  readonly items: readonly SceneGraphItem[];
  readonly streams: SourceStreams;
  readonly width?: number;
  readonly height?: number;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const videosRef = useRef<Map<CaptureKind, HTMLVideoElement>>(new Map());
  const mediaElementsRef = useRef<Map<string, SourceMediaElement>>(new Map());
  const gpuRendererRef = useRef<GpuSceneRenderer | null>(null);
  const [capture, setCapture] = useState<CanvasCaptureState>({
    supported: false,
    active: false,
    tracks: [],
    stream: null,
    backend: "canvas_2d",
    label: "preview",
  });
  const rects = useMemo(() => renderRects(items, width, height), [items, width, height]);

  useEffect(() => {
    const videos = videosRef.current;
    attachVideo(videos, "camera", streams.camera?.stream ?? null);
    attachVideo(videos, "display", streams.display?.stream ?? null);
    return () => {
      videos.forEach((video) => {
        video.pause();
        video.srcObject = null;
      });
    };
  }, [streams.camera?.stream, streams.display?.stream]);

  useEffect(() => {
    syncSourceMedia(mediaElementsRef.current, rects, window.location.href);
    return () => {
      mediaElementsRef.current.forEach((media) => {
        if (media instanceof HTMLVideoElement) {
          media.pause();
          media.removeAttribute("src");
          media.load();
        }
      });
      mediaElementsRef.current.clear();
    };
  }, [rects]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    canvas.width = width;
    canvas.height = height;
    const gpuRenderer = createGpuSceneRenderer(canvas);
    gpuRendererRef.current = gpuRenderer;
    const context = gpuRenderer ? null : canvas.getContext("2d");
    if (!gpuRenderer && !context) return;

    let frame = 0;
    const draw = () => {
      if (gpuRenderer) {
        gpuRenderer.render(rects, streams, videosRef.current, mediaElementsRef.current);
      } else if (context) {
        drawScene(context, rects, streams, videosRef.current, mediaElementsRef.current);
      }
      frame = requestAnimationFrame(draw);
    };
    draw();
    return () => {
      cancelAnimationFrame(frame);
      gpuRenderer?.destroy();
      if (gpuRendererRef.current === gpuRenderer) gpuRendererRef.current = null;
    };
  }, [rects, streams, width, height]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const captureStream = canvas?.captureStream;
    const backend = gpuRendererRef.current ? "webgl_gpu" : "canvas_2d";
    if (!canvas || !captureStream) {
      setCapture({
        supported: false,
        active: false,
        tracks: [],
        stream: null,
        backend,
        label: compositorBackendLabel(backend, false),
      });
      return;
    }
    const stream = captureStream.call(canvas, 30);
    setCapture({
      supported: true,
      active: true,
      stream,
      tracks: stream.getTracks().map((track) => `${track.kind}:${track.readyState}`),
      backend,
      label: compositorBackendLabel(backend, true),
    });
    return () => {
      stream.getTracks().forEach((track) => track.stop());
    };
  }, []);

  return {
    canvasRef,
    capture,
  };
}

function syncSourceMedia(
  elements: Map<string, SourceMediaElement>,
  rects: readonly RenderRect[],
  baseHref: string,
): void {
  const activeIds = new Set(rects.map((rect) => rect.id));
  elements.forEach((media, id) => {
    if (!activeIds.has(id)) {
      if (media instanceof HTMLVideoElement) media.pause();
      elements.delete(id);
    }
  });

  for (const rect of rects) {
    const sourceUrl = canvasDrawableMediaUrl(rect.mediaUrl, baseHref);
    if (!sourceUrl || (rect.tone !== "image" && rect.tone !== "media")) {
      elements.delete(rect.id);
      continue;
    }
    const current = elements.get(rect.id);
    if (current && current.getAttribute("src") === sourceUrl) continue;
    const next = rect.tone === "image" ? document.createElement("img") : document.createElement("video");
    next.crossOrigin = "anonymous";
    next.setAttribute("src", sourceUrl);
    if (next instanceof HTMLVideoElement) {
      next.muted = true;
      next.loop = true;
      next.playsInline = true;
      void next.play().catch(() => undefined);
    }
    elements.set(rect.id, next);
  }
}

function attachVideo(
  videos: Map<CaptureKind, HTMLVideoElement>,
  kind: CaptureKind,
  stream: MediaStream | null,
): void {
  let video = videos.get(kind);
  if (!video) {
    video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    videos.set(kind, video);
  }
  if (video.srcObject === stream) return;
  video.srcObject = stream;
  if (stream) void video.play().catch(() => undefined);
}
