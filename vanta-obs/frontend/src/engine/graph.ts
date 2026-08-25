import type { ObsRow } from "@/types";
import { boolish, num, text } from "@/types";

export interface SceneGraphItem {
  readonly id: string;
  readonly source: ObsRow;
  readonly sourceKind: string;
  readonly displayName: string;
  readonly leftPct: number;
  readonly topPct: number;
  readonly widthPct: number;
  readonly heightPct: number;
  readonly opacity: number;
  readonly zIndex: number;
}

const CANVAS_WIDTH = 1920;
const CANVAS_HEIGHT = 1080;

export function sceneGraphItems(
  instances: readonly ObsRow[],
  sources: readonly ObsRow[],
  sceneId?: string,
): readonly SceneGraphItem[] {
  return [...flattenScene(instances, sources, sceneId, parentFrame(), 0, new Set())]
    .sort((a, b) => a.zIndex - b.zIndex);
}

function flattenScene(
  instances: readonly ObsRow[],
  sources: readonly ObsRow[],
  sceneId: string | undefined,
  frame: Frame,
  zBase: number,
  seen: Set<string>,
): readonly SceneGraphItem[] {
  if (sceneId && seen.has(sceneId)) return [];
  const nextSeen = new Set(seen);
  if (sceneId) nextSeen.add(sceneId);
  return instances
    .filter((instance) => boolish(instance, "visible"))
    .filter((instance) => !sceneId || text(instance, "scene_id") === sceneId)
    .flatMap((instance) => {
      const source = sources.find((item) => item.id === text(instance, "source_id"));
      if (!source) return [];
      const childFrame = scaleFrame(instance, frame);
      const sourceKind = text(source, "source_kind");
      const zIndex = zBase + num(instance, "order_index");
      if (sourceKind === "scene_group") {
        const nestedSceneId = text(source.default_settings_json as ObsRow | undefined, "scene_id");
        return flattenScene(instances, sources, nestedSceneId, childFrame, zIndex * 100, nextSeen);
      }
      return [{
        id: instance.id,
        source,
        sourceKind,
        displayName: text(source, "display_name"),
        leftPct: childFrame.leftPct,
        topPct: childFrame.topPct,
        widthPct: childFrame.widthPct,
        heightPct: childFrame.heightPct,
        opacity: frame.opacity * num(instance, "opacity", 1),
        zIndex,
      } satisfies SceneGraphItem];
    });
}

interface Frame {
  readonly leftPct: number;
  readonly topPct: number;
  readonly widthPct: number;
  readonly heightPct: number;
  readonly opacity: number;
}

function parentFrame(): Frame {
  return { leftPct: 0, topPct: 0, widthPct: 100, heightPct: 100, opacity: 1 };
}

function scaleFrame(instance: ObsRow, parent: Frame): Frame {
  const leftPct = parent.leftPct + parent.widthPct * (num(instance, "x") / CANVAS_WIDTH);
  const topPct = parent.topPct + parent.heightPct * (num(instance, "y") / CANVAS_HEIGHT);
  const widthPct = parent.widthPct * (num(instance, "width") / CANVAS_WIDTH);
  const heightPct = parent.heightPct * (num(instance, "height") / CANVAS_HEIGHT);
  return {
    leftPct,
    topPct,
    widthPct,
    heightPct,
    opacity: parent.opacity * num(instance, "opacity", 1),
  };
}
