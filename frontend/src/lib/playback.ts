import { requestJson } from "@/lib/api";
import type { PlaybackGrant } from "@/types";

export async function preparePlaybackGrantMediaAuthorization(
  grant: PlaybackGrant,
  signal?: AbortSignal,
): Promise<void> {
  const cookieUrl = grant.mediaAuthorization.cdnCookieUrl;
  if (!cookieUrl || grant.mediaAuthorization.strategy !== "cdnSignedCookie") return;
  await requestJson<void>(cookieUrl, {
    auth: false,
    signal,
    credentials: "include",
  });
}
