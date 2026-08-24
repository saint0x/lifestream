import { useEffect, useRef } from "react";
import { useLocation } from "react-router-dom";
import { trackViewerEvent } from "@/lib/analytics";

function elementLabel(element: Element): string | undefined {
  const explicit = element.getAttribute("aria-label") ?? element.getAttribute("title");
  if (explicit?.trim()) return explicit.trim().slice(0, 160);
  const text = element.textContent?.replace(/\s+/g, " ").trim();
  return text ? text.slice(0, 160) : undefined;
}

export function ViewerAnalytics() {
  const location = useLocation();
  const enteredAt = useRef(Date.now());
  const lastPath = useRef(`${location.pathname}${location.search}`);

  useEffect(() => {
    const now = Date.now();
    const previousPath = lastPath.current;
    if (previousPath !== `${location.pathname}${location.search}`) {
      trackViewerEvent({
        eventType: "page_leave",
        path: previousPath,
        watchTimeMs: now - enteredAt.current,
      });
    }

    enteredAt.current = now;
    lastPath.current = `${location.pathname}${location.search}`;
    trackViewerEvent({
      eventType: "page_view",
      path: lastPath.current,
      metadata: { hash: location.hash || undefined },
    });
  }, [location.hash, location.pathname, location.search]);

  useEffect(() => {
    const onClick = (event: MouseEvent) => {
      const target = event.target instanceof Element
        ? event.target.closest("a,button,[role='button']")
        : null;
      if (!target) return;
      trackViewerEvent({
        eventType: "ui_click",
        metadata: {
          tag: target.tagName.toLowerCase(),
          label: elementLabel(target),
          href: target instanceof HTMLAnchorElement ? target.href : undefined,
        },
      });
    };

    const onPageHide = () => {
      trackViewerEvent({
        eventType: "page_leave",
        path: lastPath.current,
        watchTimeMs: Date.now() - enteredAt.current,
        metadata: { terminal: true },
      });
    };

    const onVisibilityChange = () => {
      if (document.visibilityState === "hidden") onPageHide();
    };

    document.addEventListener("click", onClick, { capture: true });
    window.addEventListener("pagehide", onPageHide);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      document.removeEventListener("click", onClick, { capture: true });
      window.removeEventListener("pagehide", onPageHide);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, []);

  return null;
}
