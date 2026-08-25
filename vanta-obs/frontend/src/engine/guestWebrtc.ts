export type GuestWebrtcOfferPayload = {
  readonly session_role: "guest_publish" | "guest_return" | "shared_feed_return";
  readonly direction: "sendrecv" | "sendonly" | "recvonly";
  readonly offer_sdp: string;
  readonly audio: boolean;
  readonly video: boolean;
  readonly preferred_video_layer?: string;
  readonly tracks_json?: Record<string, unknown>;
};

export type GuestWebrtcOfferOptions = {
  readonly participantId: string;
  readonly audio?: boolean;
  readonly video?: boolean;
  readonly preferredVideoLayer?: string;
};

export async function createGuestWebrtcOfferPayload(
  options: GuestWebrtcOfferOptions,
): Promise<GuestWebrtcOfferPayload> {
  const audio = options.audio ?? true;
  const video = options.video ?? true;
  if (!audio && !video) {
    throw new Error("Guest WebRTC offer requires audio or video");
  }
  const PeerConnection = globalThis.RTCPeerConnection;
  if (!PeerConnection) {
    throw new Error("WebRTC is unavailable in this browser");
  }
  const connection = new PeerConnection({
    iceServers: [{ urls: ["stun:stun.l.google.com:19302"] }],
  });
  try {
    if (audio) connection.addTransceiver("audio", { direction: "sendrecv" });
    if (video) connection.addTransceiver("video", { direction: "sendrecv" });
    const offer = await connection.createOffer({
      offerToReceiveAudio: audio,
      offerToReceiveVideo: video,
    });
    await connection.setLocalDescription(offer);
    const sdp = connection.localDescription?.sdp ?? offer.sdp ?? "";
    if (!looksLikeSdp(sdp)) {
      throw new Error("Browser returned an invalid WebRTC offer");
    }
    return {
      session_role: "guest_publish",
      direction: "sendrecv",
      offer_sdp: sdp,
      audio,
      video,
      preferred_video_layer: options.preferredVideoLayer ?? "720p30",
      tracks_json: {
        participant_id: options.participantId,
        audio,
        video,
        requested_at_ms: Date.now(),
      },
    };
  } finally {
    connection.getSenders().forEach((sender) => sender.track?.stop());
    connection.close();
  }
}

export function looksLikeSdp(value: string): boolean {
  return value.length > 0 && value.length <= 128_000 && value.includes("v=0") && value.includes("m=");
}

export function looksLikeIceCandidate(value: string): boolean {
  return value.startsWith("candidate:") && value.length <= 4096;
}
