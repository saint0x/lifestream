import { useEffect, useRef } from "react";
import { Badge } from "@/components/ui/Badge";
import { sceneGraphItems } from "@/engine/graph";
import { SourceRenderer } from "@/engine/renderers";
import { useCompositorCanvas } from "@/engine/useCompositor";
import type { CompositorBackend, SourceStreams } from "@/engine/compositor";
import type { ObsRow } from "@/types";
import { text } from "@/types";

export function ProgramCanvas({
  title,
  scene,
  instances,
  allInstances,
  sources,
  live = false,
  compact = false,
  streams = {},
  runtimeFrameSessionId = null,
  onRuntimeFrame,
}: {
  readonly title: string;
  readonly scene: ObsRow | null;
  readonly instances: readonly ObsRow[];
  readonly allInstances?: readonly ObsRow[];
  readonly sources: readonly ObsRow[];
  readonly live?: boolean;
  readonly compact?: boolean;
  readonly streams?: SourceStreams;
  readonly runtimeFrameSessionId?: string | null;
  readonly onRuntimeFrame?: (
    sessionId: string,
    imageDataUrl: string,
    compositorBackend: CompositorBackend,
    frameSequence: number,
  ) => Promise<void> | void;
}) {
  const items = sceneGraphItems(allInstances ?? instances, sources, scene?.id);
  const compositor = useCompositorCanvas({ items, streams });
  const submittedSessionRef = useRef<string | null>(null);
  useEffect(() => {
    if (!runtimeFrameSessionId || !onRuntimeFrame || title !== "Program") return;
    if (submittedSessionRef.current === runtimeFrameSessionId) return;
    submittedSessionRef.current = runtimeFrameSessionId;
    const timer = window.setTimeout(() => {
      const canvas = compositor.canvasRef.current;
      if (!canvas) return;
      try {
        void Promise.resolve(
          onRuntimeFrame(
            runtimeFrameSessionId,
            canvas.toDataURL("image/png"),
            compositor.capture.backend,
            Date.now(),
          ),
        ).catch(() => {
          submittedSessionRef.current = null;
        });
      } catch {
        submittedSessionRef.current = null;
      }
    }, 250);
    return () => window.clearTimeout(timer);
  }, [compositor.capture.backend, compositor.canvasRef, onRuntimeFrame, runtimeFrameSessionId, title]);
  return (
    <div className={`obs-canvas ${compact ? "obs-canvas--compact" : ""}`}>
      <div className="obs-canvas__surface">
        <canvas ref={compositor.canvasRef} className="obs-canvas__feed" aria-label={`${title} canvas feed`} />
        {items.map((item) => <SourceRenderer key={item.id} item={item} />)}
        <div className="obs-canvas__chrome">
          <Badge tone={live ? "live" : "new"}>{title}</Badge>
          <span className="mono">
            {text(scene, "name")} / {compositor.capture.label}
          </span>
        </div>
      </div>
    </div>
  );
}
