import type { CaptureKind, CaptureSession } from "./devices";
import type { SceneGraphItem } from "./graph";
import { sourceRendererModel } from "./renderers";

export interface RenderRect {
  readonly id: string;
  readonly sourceKind: string;
  readonly displayName: string;
  readonly detail: string;
  readonly tone: string;
  readonly mediaUrl: string;
  readonly color: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly opacity: number;
  readonly zIndex: number;
}

export interface RenderSource {
  readonly kind: CaptureKind;
  readonly stream: MediaStream | null;
}

export type SourceStreams = Partial<Record<CaptureKind, CaptureSession>>;
export type SourceMediaElement = HTMLImageElement | HTMLVideoElement;
export type CompositorBackend = "webgl_gpu" | "canvas_2d";

export interface GpuSceneRenderer {
  readonly backend: "webgl_gpu";
  render(
    rects: readonly RenderRect[],
    streams: SourceStreams,
    videoElements: ReadonlyMap<CaptureKind, HTMLVideoElement>,
    mediaElements?: ReadonlyMap<string, SourceMediaElement>,
  ): void;
  destroy(): void;
}

export function renderRects(
  items: readonly SceneGraphItem[],
  canvasWidth: number,
  canvasHeight: number,
): readonly RenderRect[] {
  return items.map((item) => {
    const model = sourceRendererModel(item.source);
    return {
      id: item.id,
      sourceKind: item.sourceKind,
      displayName: model.label || item.displayName,
      detail: model.detail,
      tone: model.tone,
      mediaUrl: model.mediaUrl,
      color: model.color,
      x: pct(item.leftPct, canvasWidth),
      y: pct(item.topPct, canvasHeight),
      width: pct(item.widthPct, canvasWidth),
      height: pct(item.heightPct, canvasHeight),
      opacity: clamp(item.opacity, 0, 1),
      zIndex: item.zIndex,
    };
  });
}

export function streamForSource(sourceKind: string, streams: SourceStreams): RenderSource | null {
  if (sourceKind === "camera") return { kind: "camera", stream: streams.camera?.stream ?? null };
  if (sourceKind === "screen_capture" || sourceKind === "display_capture") {
    return { kind: "display", stream: streams.display?.stream ?? null };
  }
  return null;
}

export function drawScene(
  context: CanvasRenderingContext2D,
  rects: readonly RenderRect[],
  streams: SourceStreams,
  videoElements: ReadonlyMap<CaptureKind, HTMLVideoElement>,
  mediaElements: ReadonlyMap<string, SourceMediaElement> = new Map(),
): void {
  const { width, height } = context.canvas;
  context.clearRect(0, 0, width, height);
  drawBackdrop(context, width, height);
  for (const rect of rects) {
    context.save();
    context.globalAlpha = rect.opacity;
    const source = streamForSource(rect.sourceKind, streams);
    const video = source ? videoElements.get(source.kind) : null;
    const media = mediaElements.get(rect.id);
    if (source?.stream && video?.readyState && video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) {
      context.drawImage(video, rect.x, rect.y, rect.width, rect.height);
    } else if (isDrawableSourceMedia(rect, media)) {
      context.drawImage(media, rect.x, rect.y, rect.width, rect.height);
    } else {
      drawSourcePlate(context, rect);
    }
    drawSourceLabel(context, rect);
    context.restore();
  }
}

export function createGpuSceneRenderer(canvas: HTMLCanvasElement): GpuSceneRenderer | null {
  const gl = canvas.getContext("webgl2", { alpha: false }) ?? canvas.getContext("webgl", { alpha: false });
  if (!gl) return null;
  const program = createProgram(gl);
  if (!program) return null;
  const position = gl.getAttribLocation(program, "a_position");
  const texCoord = gl.getAttribLocation(program, "a_texCoord");
  const resolution = gl.getUniformLocation(program, "u_resolution");
  const color = gl.getUniformLocation(program, "u_color");
  const useTexture = gl.getUniformLocation(program, "u_useTexture");
  const textureSampler = gl.getUniformLocation(program, "u_texture");
  const alpha = gl.getUniformLocation(program, "u_alpha");
  const buffer = gl.createBuffer();
  const texCoordBuffer = gl.createBuffer();
  const texture = gl.createTexture();
  if (
    position < 0
    || texCoord < 0
    || !resolution
    || !color
    || !useTexture
    || !textureSampler
    || !alpha
    || !buffer
    || !texCoordBuffer
    || !texture
  ) {
    return null;
  }

  gl.useProgram(program);
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.uniform1i(textureSampler, 0);

  return {
    backend: "webgl_gpu",
    render(rects, streams, videoElements, mediaElements = new Map()) {
      gl.viewport(0, 0, gl.canvas.width, gl.canvas.height);
      gl.clearColor(0.02, 0.02, 0.02, 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.useProgram(program);
      gl.uniform2f(resolution, gl.canvas.width, gl.canvas.height);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

      for (const rect of rects) {
        const source = streamForSource(rect.sourceKind, streams);
        const video = source ? videoElements.get(source.kind) : null;
        const media = mediaElements.get(rect.id);
        const drawable = drawableGpuMedia(rect, video, media);
        drawGpuQuad(gl, buffer, texCoordBuffer, position, texCoord, rect);
        gl.uniform1f(alpha, rect.opacity);
        if (drawable) {
          gl.activeTexture(gl.TEXTURE0);
          gl.bindTexture(gl.TEXTURE_2D, texture);
          gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
          gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, drawable);
          gl.uniform1i(useTexture, 1);
          gl.uniform4f(color, 1, 1, 1, 1);
        } else {
          const rgba = rgbaFor(rect);
          gl.uniform1i(useTexture, 0);
          gl.uniform4f(color, rgba[0], rgba[1], rgba[2], rgba[3]);
        }
        gl.drawArrays(gl.TRIANGLES, 0, 6);
      }
    },
    destroy() {
      gl.deleteTexture(texture);
      gl.deleteBuffer(buffer);
      gl.deleteBuffer(texCoordBuffer);
      gl.deleteProgram(program);
    },
  };
}

export function compositorBackendLabel(backend: CompositorBackend, captureSupported: boolean): string {
  if (backend === "webgl_gpu") return captureSupported ? "gpu capture" : "gpu preview";
  return captureSupported ? "capture" : "preview";
}

export function canvasDrawableMediaUrl(mediaUrl: string, baseHref: string): string {
  if (!mediaUrl.trim()) return "";
  if (mediaUrl.startsWith("/") || mediaUrl.startsWith("./") || mediaUrl.startsWith("../")) return mediaUrl;
  if (mediaUrl.startsWith("blob:") || mediaUrl.startsWith("data:")) return mediaUrl;
  try {
    const url = new URL(mediaUrl, baseHref);
    const base = new URL(baseHref);
    return url.origin === base.origin ? url.href : "";
  } catch {
    return "";
  }
}

function isDrawableSourceMedia(rect: RenderRect, media: SourceMediaElement | undefined): media is SourceMediaElement {
  if (!media || !rect.mediaUrl) return false;
  if (rect.tone === "image") {
    return media instanceof HTMLImageElement && media.complete && media.naturalWidth > 0;
  }
  if (rect.tone === "media") {
    return media instanceof HTMLVideoElement && media.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA;
  }
  return false;
}

function drawBackdrop(context: CanvasRenderingContext2D, width: number, height: number): void {
  const gradient = context.createLinearGradient(0, 0, width, height);
  gradient.addColorStop(0, "#060606");
  gradient.addColorStop(0.58, "#111111");
  gradient.addColorStop(1, "#020202");
  context.fillStyle = gradient;
  context.fillRect(0, 0, width, height);

  context.strokeStyle = "rgba(255,255,255,0.045)";
  context.lineWidth = 1;
  for (let x = 0; x <= width; x += 80) {
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, height);
    context.stroke();
  }
  for (let y = 0; y <= height; y += 80) {
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(width, y);
    context.stroke();
  }
}

function drawSourcePlate(context: CanvasRenderingContext2D, rect: RenderRect): void {
  context.fillStyle = rect.tone === "matte" && rect.color ? rect.color : fillFor(rect.sourceKind, rect.tone);
  context.strokeStyle = rect.tone === "guide" ? "rgba(255,255,255,0.55)" : "rgba(255,255,255,0.24)";
  context.lineWidth = 2;
  context.fillRect(rect.x, rect.y, rect.width, rect.height);
  if (rect.tone === "guide") {
    drawSafeArea(context, rect);
  }
  context.strokeRect(rect.x, rect.y, rect.width, rect.height);
}

function drawSourceLabel(context: CanvasRenderingContext2D, rect: RenderRect): void {
  const labelWidth = Math.min(rect.width - 24, Math.max(120, rect.displayName.length * 7 + 24));
  if (labelWidth <= 0 || rect.height < 28) return;
  context.fillStyle = "rgba(0,0,0,0.58)";
  context.fillRect(rect.x + 12, rect.y + 12, labelWidth, 24);
  context.fillStyle = "rgba(255,255,255,0.92)";
  context.font = "12px Inter, system-ui, sans-serif";
  context.fillText(rect.displayName, rect.x + 22, rect.y + 28, labelWidth - 20);
  if (rect.detail && rect.height >= 48) {
    context.fillStyle = "rgba(255,255,255,0.68)";
    context.font = "10px Inter, system-ui, sans-serif";
    context.fillText(rect.detail.toUpperCase(), rect.x + 22, rect.y + 44, labelWidth - 20);
  }
}

function drawSafeArea(context: CanvasRenderingContext2D, rect: RenderRect): void {
  const x = rect.x + rect.width * 0.05;
  const y = rect.y + rect.height * 0.05;
  const width = rect.width * 0.9;
  const height = rect.height * 0.9;
  context.strokeStyle = "rgba(255,255,255,0.42)";
  context.setLineDash([8, 8]);
  context.strokeRect(x, y, width, height);
  context.setLineDash([]);
}

function fillFor(sourceKind: string, tone: string): string {
  if (tone === "audio") return "rgba(61,220,151,0.16)";
  if (tone === "screen" || tone === "browser") return "rgba(78,161,255,0.16)";
  if (tone === "sponsor" || tone === "brand" || tone === "cta" || tone === "promo" || tone === "qr") return "rgba(255,204,61,0.18)";
  if (tone === "guest" || sourceKind === "guest_feed") return "rgba(114,92,255,0.18)";
  if (tone === "timer" || tone === "alert") return "rgba(255,45,85,0.18)";
  if (tone === "image" || tone === "media") return "rgba(255,255,255,0.11)";
  if (tone === "text" || tone === "lower") return "rgba(0,0,0,0.22)";
  if (sourceKind === "camera") return "rgba(255,255,255,0.12)";
  return "rgba(255,255,255,0.075)";
}

function drawableGpuMedia(
  rect: RenderRect,
  video: HTMLVideoElement | undefined | null,
  media: SourceMediaElement | undefined,
): HTMLImageElement | HTMLVideoElement | null {
  if (video?.readyState && video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) return video;
  if (isDrawableSourceMedia(rect, media)) return media;
  return null;
}

function drawGpuQuad(
  gl: WebGLRenderingContext | WebGL2RenderingContext,
  buffer: WebGLBuffer,
  texCoordBuffer: WebGLBuffer,
  position: number,
  texCoord: number,
  rect: RenderRect,
): void {
  const x1 = rect.x;
  const y1 = rect.y;
  const x2 = rect.x + rect.width;
  const y2 = rect.y + rect.height;
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([x1, y1, x2, y1, x1, y2, x1, y2, x2, y1, x2, y2]),
    gl.STREAM_DRAW,
  );
  gl.enableVertexAttribArray(position);
  gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
  gl.bindBuffer(gl.ARRAY_BUFFER, texCoordBuffer);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]),
    gl.STATIC_DRAW,
  );
  gl.enableVertexAttribArray(texCoord);
  gl.vertexAttribPointer(texCoord, 2, gl.FLOAT, false, 0, 0);
}

function createProgram(gl: WebGLRenderingContext | WebGL2RenderingContext): WebGLProgram | null {
  const vertex = compileShader(gl, gl.VERTEX_SHADER, `
    attribute vec2 a_position;
    attribute vec2 a_texCoord;
    uniform vec2 u_resolution;
    varying vec2 v_texCoord;
    void main() {
      vec2 zeroToOne = a_position / u_resolution;
      vec2 clipSpace = zeroToOne * 2.0 - 1.0;
      gl_Position = vec4(clipSpace * vec2(1, -1), 0, 1);
      v_texCoord = a_texCoord;
    }
  `);
  const fragment = compileShader(gl, gl.FRAGMENT_SHADER, `
    precision mediump float;
    uniform sampler2D u_texture;
    uniform vec4 u_color;
    uniform bool u_useTexture;
    uniform float u_alpha;
    varying vec2 v_texCoord;
    void main() {
      vec4 source = u_useTexture ? texture2D(u_texture, v_texCoord) : u_color;
      gl_FragColor = vec4(source.rgb, source.a * u_alpha);
    }
  `);
  if (!vertex || !fragment) return null;
  const program = gl.createProgram();
  if (!program) return null;
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    gl.deleteProgram(program);
    return null;
  }
  return program;
}

function compileShader(
  gl: WebGLRenderingContext | WebGL2RenderingContext,
  type: number,
  source: string,
): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

function rgbaFor(rect: RenderRect): readonly [number, number, number, number] {
  const source = rect.tone === "matte" && rect.color ? rect.color : fillFor(rect.sourceKind, rect.tone);
  const match = /rgba?\(([^)]+)\)/.exec(source);
  const raw = match?.[1];
  if (!raw) return [0.08, 0.08, 0.08, 1];
  const parts = raw.split(",").map((part) => Number(part.trim()));
  return [
    (parts[0] ?? 20) / 255,
    (parts[1] ?? 20) / 255,
    (parts[2] ?? 20) / 255,
    parts[3] ?? 1,
  ];
}

function pct(value: number, size: number): number {
  return (value / 100) * size;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
