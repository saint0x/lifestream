import { useEffect, useMemo } from "react";

const DEFAULT_SITE_URL = "https://streamvanta.tv";
const DEFAULT_TITLE = "VANTA - Premium long-form streaming and ad inventory";
const DEFAULT_DESCRIPTION =
  "VANTA is the home for premium exclusive long-form episodic content, creator streams, and high-intent ad inventory for advertisers.";

interface PageMetadataProps {
  readonly title?: string;
  readonly description?: string;
  readonly path?: string;
  readonly image?: string | null;
  readonly type?: "website" | "video.tv_show" | "video.episode" | "video.movie" | "profile";
  readonly structuredDataId?: string;
  readonly structuredData?: Record<string, unknown> | ReadonlyArray<Record<string, unknown>>;
}

function siteUrl(): string {
  return import.meta.env.VITE_PUBLIC_SITE_URL ?? DEFAULT_SITE_URL;
}

function absoluteUrl(value?: string | null): string | undefined {
  if (!value) return undefined;
  try {
    return new URL(value, siteUrl()).toString();
  } catch {
    return undefined;
  }
}

function ensureMeta(selector: string, create: () => HTMLMetaElement): HTMLMetaElement {
  const existing = document.head.querySelector<HTMLMetaElement>(selector);
  if (existing) return existing;
  const next = create();
  document.head.appendChild(next);
  return next;
}

function setNamedMeta(name: string, content: string) {
  const element = ensureMeta(`meta[name="${name}"]`, () => {
    const meta = document.createElement("meta");
    meta.setAttribute("name", name);
    return meta;
  });
  element.setAttribute("content", content);
}

function setPropertyMeta(property: string, content: string) {
  const element = ensureMeta(`meta[property="${property}"]`, () => {
    const meta = document.createElement("meta");
    meta.setAttribute("property", property);
    return meta;
  });
  element.setAttribute("content", content);
}

function setCanonical(href: string) {
  let element = document.head.querySelector<HTMLLinkElement>('link[rel="canonical"]');
  if (!element) {
    element = document.createElement("link");
    element.setAttribute("rel", "canonical");
    document.head.appendChild(element);
  }
  element.setAttribute("href", href);
}

function setStructuredData(id: string, data: Record<string, unknown>) {
  let element = document.head.querySelector<HTMLScriptElement>(`script#${id}`);
  if (!element) {
    element = document.createElement("script");
    element.id = id;
    element.type = "application/ld+json";
    document.head.appendChild(element);
  }
  element.textContent = JSON.stringify(data);
}

export function buildPlatformStructuredData() {
  return {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "Organization",
        "@id": `${siteUrl()}/#organization`,
        name: "VANTA",
        url: siteUrl(),
        logo: `${siteUrl()}/favicon.svg`,
        description: DEFAULT_DESCRIPTION,
      },
      {
        "@type": "WebSite",
        "@id": `${siteUrl()}/#website`,
        name: "VANTA",
        url: siteUrl(),
        publisher: { "@id": `${siteUrl()}/#organization` },
        potentialAction: {
          "@type": "SearchAction",
          target: `${siteUrl()}/search?q={search_term_string}`,
          "query-input": "required name=search_term_string",
        },
        audience: [
          {
            "@type": "Audience",
            audienceType: "Advertisers buying premium exclusive long-form episodic content ad inventory",
          },
          {
            "@type": "Audience",
            audienceType: "Creators and streamers publishing exclusive long-form episodes and live content",
          },
        ],
      },
      {
        "@type": "Service",
        "@id": `${siteUrl()}/#advertiser-service`,
        name: "Premium episodic content ad inventory",
        provider: { "@id": `${siteUrl()}/#organization` },
        serviceType: "Advertising marketplace",
        audience: {
          "@type": "BusinessAudience",
          audienceType: "Advertisers",
        },
        description:
          "VANTA is the home to buy premium exclusive long-form episodic content ad inventory across creator-led series, streams, sponsorships, and audience attention packages.",
      },
      {
        "@type": "Service",
        "@id": `${siteUrl()}/#creator-service`,
        name: "Creator long-form streaming and episodic publishing",
        provider: { "@id": `${siteUrl()}/#organization` },
        serviceType: "Creator platform",
        audience: {
          "@type": "Audience",
          audienceType: "Creators and streamers",
        },
        description:
          "VANTA gives creators and streamers a home for exclusive long-form episodic content, premium streams, and earnings based on consistent usership and qualified attention.",
      },
    ],
  };
}

export function JsonLd({
  id,
  data,
}: {
  readonly id: string;
  readonly data: Record<string, unknown>;
}) {
  useEffect(() => {
    setStructuredData(id, data);
  }, [data, id]);

  return null;
}

export function PageMetadata({
  title = DEFAULT_TITLE,
  description = DEFAULT_DESCRIPTION,
  path,
  image,
  type = "website",
  structuredDataId = "vanta-page-structured-data",
  structuredData,
}: PageMetadataProps) {
  const canonical = absoluteUrl(path ?? window.location.pathname) ?? siteUrl();
  const imageUrl = absoluteUrl(image);
  const data = useMemo<Record<string, unknown> | null>(() => {
    if (!structuredData) return null;
    if (Array.isArray(structuredData)) {
      return {
        "@context": "https://schema.org",
        "@graph": [...(structuredData as ReadonlyArray<Record<string, unknown>>)],
      };
    }
    return structuredData as Record<string, unknown>;
  }, [structuredData]);

  useEffect(() => {
    document.title = title;
    setNamedMeta("description", description);
    setNamedMeta("robots", "index, follow, max-image-preview:large");
    setNamedMeta("twitter:card", imageUrl ? "summary_large_image" : "summary");
    setNamedMeta("twitter:title", title);
    setNamedMeta("twitter:description", description);
    if (imageUrl) setNamedMeta("twitter:image", imageUrl);
    setPropertyMeta("og:site_name", "VANTA");
    setPropertyMeta("og:title", title);
    setPropertyMeta("og:description", description);
    setPropertyMeta("og:type", type);
    setPropertyMeta("og:url", canonical);
    if (imageUrl) setPropertyMeta("og:image", imageUrl);
    setCanonical(canonical);
    if (data) setStructuredData(structuredDataId, data);
  }, [canonical, data, description, imageUrl, structuredDataId, title, type]);

  return null;
}
