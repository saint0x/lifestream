import type { Category } from "@/types";
import { img } from "./images";

export const categories: ReadonlyArray<Category> = [
  {
    slug: "drama",
    name: "Drama",
    coverImage: img.square("cat-drama"),
    liveViewers: 12_450,
    liveChannels: 312,
    tags: ["prestige", "slow-burn", "character"],
  },
  {
    slug: "tech",
    name: "Tech",
    coverImage: img.square("cat-tech"),
    liveViewers: 38_221,
    liveChannels: 540,
    tags: ["rust", "systems", "web", "linux"],
  },
  {
    slug: "gaming",
    name: "Gaming",
    coverImage: img.square("cat-gaming"),
    liveViewers: 212_080,
    liveChannels: 6_720,
    tags: ["speedrun", "horror", "indie", "ranked"],
  },
  {
    slug: "music",
    name: "Music",
    coverImage: img.square("cat-music"),
    liveViewers: 18_092,
    liveChannels: 201,
    tags: ["jazz", "ambient", "synth", "piano"],
  },
  {
    slug: "talk",
    name: "Talk",
    coverImage: img.square("cat-talk"),
    liveViewers: 44_550,
    liveChannels: 120,
    tags: ["podcast", "interview", "late-night"],
  },
  {
    slug: "sports",
    name: "Sports",
    coverImage: img.square("cat-sports"),
    liveViewers: 88_401,
    liveChannels: 230,
    tags: ["racing", "esports", "analysis"],
  },
  {
    slug: "sci-fi",
    name: "Sci-Fi",
    coverImage: img.square("cat-scifi"),
    liveViewers: 9_220,
    liveChannels: 88,
    tags: ["space", "future", "hard"],
  },
  {
    slug: "documentary",
    name: "Documentary",
    coverImage: img.square("cat-doc"),
    liveViewers: 4_310,
    liveChannels: 42,
    tags: ["nature", "history", "science"],
  },
];
