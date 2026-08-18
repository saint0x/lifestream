import type { User } from "@/types";
import { img } from "./images";

export const currentUser: User = {
  id: "usr-1",
  handle: "deepsaint",
  displayName: "deepsaint",
  avatar: img.avatar("deepsaint"),
  tier: "premium",
  joinedAt: "2024-11-03",
  watchlist: ["ser-northlight", "flm-paper-moon", "ser-halcyon-drift"],
  following: ["str-atlas", "str-kai", "str-mira"],
  continueWatching: [
    {
      contentId: "ser-northlight",
      kind: "series",
      episodeId: "ser-northlight-s2e3",
      progressSec: 1280,
      durationSec: 3120,
      lastWatchedAt: "2026-04-11T22:10:00Z",
    },
    {
      contentId: "flm-afterglow",
      kind: "film",
      progressSec: 4210,
      durationSec: 7140,
      lastWatchedAt: "2026-04-10T20:40:00Z",
    },
    {
      contentId: "ser-the-long-quiet",
      kind: "series",
      episodeId: "ser-the-long-quiet-s1e6",
      progressSec: 400,
      durationSec: 3000,
      lastWatchedAt: "2026-04-09T01:04:00Z",
    },
    {
      contentId: "ser-halcyon-drift",
      kind: "series",
      episodeId: "ser-halcyon-drift-s1e2",
      progressSec: 2100,
      durationSec: 3000,
      lastWatchedAt: "2026-04-08T19:22:00Z",
    },
  ],
};
