import type { LiveStream } from "@/types";
import { streamers } from "./streamers";
import { img } from "./images";

const byId = (id: string) => {
  const s = streamers.find((x) => x.id === id);
  if (!s) throw new Error(`Unknown streamer: ${id}`);
  return s;
};

const now = new Date("2026-04-12T19:00:00Z");
const minutesAgo = (n: number): string =>
  new Date(now.getTime() - n * 60_000).toISOString();

export const liveStreams: ReadonlyArray<LiveStream> = [
  {
    id: "lv-atlas-rust",
    slug: "atlas-rust-async",
    kind: "live",
    title: "writing a distributed job queue in rust from scratch",
    category: "Tech",
    tags: ["rust", "tokio", "systems", "english"],
    streamer: byId("str-atlas"),
    viewers: 14_204,
    startedAt: minutesAgo(142),
    thumbnail: img.thumbnail("lv-atlas"),
    language: "EN",
    isMature: false,
  },
  {
    id: "lv-noctis-resident",
    slug: "noctis-silent-hill",
    kind: "live",
    title: "silent hill 2 — blind run, one life, bad idea",
    category: "Gaming",
    tags: ["horror", "blind", "retro"],
    streamer: byId("str-noctis"),
    viewers: 8_891,
    startedAt: minutesAgo(63),
    thumbnail: img.thumbnail("lv-noctis"),
    language: "EN",
    isMature: true,
  },
  {
    id: "lv-paper-radio",
    slug: "paper-radio-night",
    kind: "live",
    title: "paper.radio — rainy tuesday / slow jazz",
    category: "Music",
    tags: ["jazz", "ambient", "lofi"],
    streamer: byId("str-paper"),
    viewers: 3_109,
    startedAt: minutesAgo(305),
    thumbnail: img.thumbnail("lv-paper"),
    language: "EN",
    isMature: false,
  },
  {
    id: "lv-kai-hardware",
    slug: "kai-crt-doom",
    kind: "live",
    title: "soldering a tiny CRT to run tinier DOOM",
    category: "Tech",
    tags: ["hardware", "diy", "retro"],
    streamer: byId("str-kai"),
    viewers: 6_742,
    startedAt: minutesAgo(88),
    thumbnail: img.thumbnail("lv-kai"),
    language: "EN",
    isMature: false,
  },
  {
    id: "lv-mira-talk",
    slug: "mira-late-night",
    kind: "live",
    title: "late night w/ mira — guest: a competitive sleeper",
    category: "Talk",
    tags: ["interview", "conversation"],
    streamer: byId("str-mira"),
    viewers: 11_450,
    startedAt: minutesAgo(27),
    thumbnail: img.thumbnail("lv-mira"),
    language: "EN",
    isMature: false,
  },
  {
    id: "lv-vex-fg",
    slug: "vex-fighterz",
    kind: "live",
    title: "ranked grind, top 1000 or i uninstall",
    category: "Gaming",
    tags: ["fighting", "ranked"],
    streamer: byId("str-vex"),
    viewers: 2_231,
    startedAt: minutesAgo(201),
    thumbnail: img.thumbnail("lv-vex"),
    language: "EN",
    isMature: true,
  },
  {
    id: "lv-gridline-endurance",
    slug: "gridline-endurance",
    kind: "live",
    title: "24h endurance race — hour 6 — le mans prototype",
    category: "Sports",
    tags: ["racing", "sim", "endurance"],
    streamer: byId("str-gridline"),
    viewers: 22_108,
    startedAt: minutesAgo(360),
    thumbnail: img.thumbnail("lv-gridline"),
    language: "EN",
    isMature: false,
  },
];
