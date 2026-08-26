import type { LucideIcon } from "lucide-react";
import {
  BadgeDollarSign,
  BarChart3,
  Clapperboard,
  FileCheck2,
  Layers3,
  Megaphone,
  Play,
  ShieldCheck,
  Users,
  WalletCards,
} from "lucide-react";
import { cdnAsset } from "@/lib/assets";

export type Audience = "home" | "creators" | "buyers";
export type FunnelAudience = Exclude<Audience, "home">;

export type Metric = {
  readonly label: string;
  readonly value: string;
  readonly detail: string;
};

export type Proof = {
  readonly icon: LucideIcon;
  readonly title: string;
  readonly body: string;
};

export type Faq = {
  readonly question: string;
  readonly answer: string;
};

export type Step = {
  readonly label: string;
  readonly title: string;
  readonly body: string;
};

export type Tile = {
  readonly image: string;
  readonly label: string;
  readonly title: string;
  readonly icon: LucideIcon;
};

export type SignupField = {
  readonly name: "name" | "email" | "company" | "website" | "audience" | "budget" | "message";
  readonly label: string;
  readonly placeholder: string;
  readonly required?: boolean;
  readonly multiline?: boolean;
};

export type PageContent = {
  readonly audience: FunnelAudience;
  readonly eyebrow: string;
  readonly title: string;
  readonly subtitle: string;
  readonly primaryCta: string;
  readonly secondaryCta?: string;
  readonly formTitle: string;
  readonly formSubtitle: string;
  readonly formSuccess: string;
  readonly image: string;
  readonly metrics: readonly Metric[];
  readonly proof: readonly Proof[];
  readonly stepsTitle?: string;
  readonly stepsIntro?: string;
  readonly steps?: readonly Step[];
  readonly strips: readonly string[];
  readonly faq: readonly Faq[];
  readonly fields: readonly SignupField[];
  readonly tiles?: readonly Tile[];
};

const creatorFields = [
  { name: "name", label: "Name", placeholder: "Your name", required: true },
  { name: "email", label: "Email", placeholder: "you@example.com", required: true },
  { name: "website", label: "Main channel", placeholder: "Your strongest link", required: true },
  { name: "audience", label: "Audience", placeholder: "Niche, size, proof" },
  { name: "message", label: "Series", placeholder: "What would you launch on Vanta?", multiline: true },
] satisfies readonly SignupField[];

const buyerFields = [
  { name: "name", label: "Name", placeholder: "Your name", required: true },
  { name: "email", label: "Work email", placeholder: "you@brand.com", required: true },
  { name: "company", label: "Company", placeholder: "Brand or agency", required: true },
  { name: "website", label: "Website", placeholder: "https://", required: true },
  { name: "budget", label: "Budget", placeholder: "Pilot, season, or category buy" },
  { name: "message", label: "Goal", placeholder: "Audience or category you want", multiline: true },
] satisfies readonly SignupField[];

export const pages: Record<FunnelAudience, PageContent> = {
  creators: {
    audience: "creators",
    eyebrow: "For creators who can move an audience",
    title: "Create the show. Bring the audience. Get paid.",
    subtitle:
      "Vanta turns exclusive shows into ad inventory. Publish your best work, promote your link, and earn when real viewers show up.",
    primaryCta: "Sign up",
    formTitle: "Apply to join Vanta",
    formSubtitle: "Send your audience, show idea, and launch plan.",
    formSuccess: "Application received. We will review it.",
    image: cdnAsset("landing/platform-shots/details/creator-profile-detail-v2.png"),
    metrics: [],
    proof: [
      {
        icon: BadgeDollarSign,
        title: "Your best viewers should be worth more",
        body: "Bring real viewers to premium work. Vanta helps turn that demand into ad money.",
      },
      {
        icon: Layers3,
        title: "Your profile makes you easier to buy",
        body: "Your Vanta profile gives brands one serious place to see your shows, credits, links, and value.",
      },
      {
        icon: Megaphone,
        title: "Your marketing points to an asset",
        body: "Every clip, post, email, and launch push sends fans to one destination that can grow.",
      },
    ],
    stepsTitle: "Make something worth watching. Make sure people show up.",
    stepsIntro:
      "Vanta works when creators treat their best work like a show and their audience like distribution.",
    steps: [
      {
        label: "01",
        title: "Publish exclusive work",
        body: "Put the high-quality episodes, films, shows, or live formats on Vanta so fans have one clear place to watch.",
      },
      {
        label: "02",
        title: "Promote your Vanta link",
        body: "Use every channel you already have: clips, stories, email, posts, pinned links, and launch pushes.",
      },
      {
        label: "03",
        title: "Bring people back",
        body: "Train viewers around the cadence of your show, whether that is daily, weekly, live, or season by season.",
      },
      {
        label: "04",
        title: "Get paid on the value",
        body: "When the audience is real and repeatable, Vanta turns that attention into something advertisers can buy.",
      },
    ],
    strips: ["Make premium work", "Market yourself hard", "Prove real attention", "Convert it into cash"],
    faq: [
      {
        question: "What is the creator deal in plain English?",
        answer: "You bring exclusive work and real promotion. Vanta handles presentation, measurement, packaging, reporting, and renewals.",
      },
      {
        question: "How do I make money from this?",
        answer: "You earn when your audience watches, returns, and engages. That attention becomes something brands can buy.",
      },
      {
        question: "Why does the profile matter?",
        answer: "It gives brands and viewers one serious surface. Think IMDb-style portfolio for your shows, credits, and best links.",
      },
      {
        question: "Do I still need to market myself?",
        answer: "Yes. The creators who win make strong work and promote it hard. Vanta gives that effort a place to compound.",
      },
      {
        question: "Do I have to give up my other platforms?",
        answer: "No. Keep posting everywhere, amplify what already works, and point that traffic back to your Vanta profile.",
      },
      {
        question: "What if Vanta does not have network effects yet?",
        answer: "Advertisers do not need a crowded platform; one creator with a hypothetical 10,000 consistent viewers is already valuable inventory.",
      },
      {
        question: "Does it need to look like a studio production?",
        answer: "No. Big production value helps, but exclusive work is valuable when it is strong, consistent, and worth watching. Vanta gives that work a more premium home.",
      },
    ],
    fields: creatorFields,
    tiles: [],
  },
  buyers: {
    audience: "buyers",
    eyebrow: "For brands and agencies",
    title: "Buy ads inside shows people actually watch.",
    subtitle:
      "Vanta gives agencies HBO-style creator programming with 30-40 minute sessions. Qualified Attention proves the audience is real.",
    primaryCta: "Sign up",
    formTitle: "Create a buyer account",
    formSubtitle: "Share your category, budget, and goal.",
    formSuccess: "Signup received. We will follow up.",
    image: cdnAsset("landing/platform-shots/details/agency-overview-detail.png"),
    metrics: [
      { label: "Inventory", value: "Episodes", detail: "Shows, seasons, live specials" },
      { label: "Session", value: "30-40 min", detail: "Deeper than feed posts" },
      { label: "Proof", value: "Qualified", detail: "Attention scored from behavior" },
      { label: "Buy", value: "Clear", detail: "Creator, placement, reporting" },
    ],
    proof: [
      {
        icon: ShieldCheck,
        title: "Premium episodes are premium inventory",
        body: "A full episode is not a thumb-stopped impression. The context is deeper and more valuable.",
      },
      {
        icon: FileCheck2,
        title: "Qualified Attention verifies the buy",
        body: "Our algorithm shows whether viewers watched, returned, engaged, and created value.",
      },
      {
        icon: WalletCards,
        title: "Know the exact media buy",
        body: "Buy by creator, show, episode, season, category, or launch moment.",
      },
    ],
    stepsTitle: "Premium inventory, verified attention.",
    stepsIntro:
      "Pick the show, lock the context, measure the audience, scale what works.",
    steps: [
      {
        label: "01",
        title: "Pick the programming",
        body: "Choose creator shows, episodes, seasons, live events, or category slates built for longer viewing.",
      },
      {
        label: "02",
        title: "Lock the context",
        body: "Know the creator, show, placement, campaign window, format, safety terms, and promotion commitment.",
      },
      {
        label: "03",
        title: "Verify the attention",
        body: "Qualified Attention evaluates watch depth, returning viewers, engagement, attribution confidence, and traffic quality.",
      },
      {
        label: "04",
        title: "Scale what performs",
        body: "Use the report to renew the creator, expand into the season, or buy deeper into the category.",
      },
    ],
    strips: ["HBO-style episodes", "30-40 minute attention", "Bespoke QA algorithm", "Premium creator inventory"],
    faq: [
      {
        question: "Why is this valuable ad inventory?",
        answer: "Viewers choose to watch premium creator programming. A 30-40 minute episode creates more context than a short-form impression.",
      },
      {
        question: "What is Qualified Attention?",
        answer: "Qualified Attention scores audience quality. It checks watch depth, returns, engagement, attribution, and traffic quality.",
      },
      {
        question: "How is this different from influencer marketing?",
        answer: "Influencer marketing buys a post. Vanta lets you buy media inside creator-owned programming.",
      },
      {
        question: "What exactly do we know before buying?",
        answer: "Creator, category, show context, placement, timing, promotion, safety terms, and reporting.",
      },
    ],
    fields: buyerFields,
    tiles: [
      {
        image: cdnAsset("landing/platform-shots/details/agency-creators-detail.png"),
        label: "Episode inventory",
        title: "Sponsor shows viewers finish.",
        icon: Play,
      },
      {
        image: cdnAsset("landing/platform-shots/details/agency-overview-detail.png"),
        label: "Show context",
        title: "Place brands inside the show.",
        icon: Clapperboard,
      },
      {
        image: cdnAsset("landing/platform-shots/details/agency-stats-detail.png"),
        label: "Qualified Attention",
        title: "Separate signal from noise.",
        icon: ShieldCheck,
      },
      {
        image: cdnAsset("landing/platform-shots/details/series-detail-detail.png"),
        label: "Long sessions",
        title: "Buy minutes, not glances.",
        icon: BarChart3,
      },
      {
        image: cdnAsset("landing/platform-shots/details/creator-profile-detail-v2.png"),
        label: "Creator trust",
        title: "Buy creator trust in context.",
        icon: Users,
      },
      {
        image: cdnAsset("landing/platform-shots/details/agency-overview-detail.png"),
        label: "Defined buy",
        title: "Know the buy before spend.",
        icon: FileCheck2,
      },
    ],
  },
};

export const routeToAudience = (path: string): Audience => {
  if (path === "/creators") return "creators";
  if (path === "/buyers") return "buyers";
  return "home";
};

export const audienceSignupKind = (audience: Audience): "creator" | "buyer" | "general" => {
  if (audience === "creators") return "creator";
  if (audience === "buyers") return "buyer";
  return "general";
};
