import { num, type ObsRow } from "@/types";

export type SceneMoveDirection = "up" | "down";

export function orderedScenes(scenes: readonly ObsRow[]): readonly ObsRow[] {
  return [...scenes].sort((a, b) => num(a, "order_index") - num(b, "order_index"));
}

export function movedSceneIds(
  scenes: readonly ObsRow[],
  sceneId: string,
  direction: SceneMoveDirection,
): readonly string[] {
  const ordered = [...orderedScenes(scenes)];
  const index = ordered.findIndex((scene) => scene.id === sceneId);
  if (index < 0) return ordered.map((scene) => scene.id);
  const target = direction === "up" ? index - 1 : index + 1;
  if (target < 0 || target >= ordered.length) return ordered.map((scene) => scene.id);
  const current = ordered[index];
  const next = ordered[target];
  if (!current || !next) return ordered.map((scene) => scene.id);
  ordered[index] = next;
  ordered[target] = current;
  return ordered.map((scene) => scene.id);
}
