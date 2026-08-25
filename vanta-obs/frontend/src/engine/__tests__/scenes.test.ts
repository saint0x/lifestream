import { describe, expect, it } from "vitest";
import { movedSceneIds, orderedScenes } from "@/engine/scenes";

const scenes = [
  { id: "scene_b", order_index: 2 },
  { id: "scene_a", order_index: 1 },
  { id: "scene_c", order_index: 3 },
];

describe("scenes", () => {
  it("keeps scene rail order deterministic from backend order_index", () => {
    expect(orderedScenes(scenes).map((scene) => scene.id)).toEqual([
      "scene_a",
      "scene_b",
      "scene_c",
    ]);
  });

  it("moves a scene one slot without dropping ids", () => {
    expect(movedSceneIds(scenes, "scene_b", "up")).toEqual([
      "scene_b",
      "scene_a",
      "scene_c",
    ]);
    expect(movedSceneIds(scenes, "scene_b", "down")).toEqual([
      "scene_a",
      "scene_c",
      "scene_b",
    ]);
  });

  it("leaves boundary moves unchanged", () => {
    expect(movedSceneIds(scenes, "scene_a", "up")).toEqual([
      "scene_a",
      "scene_b",
      "scene_c",
    ]);
  });
});
