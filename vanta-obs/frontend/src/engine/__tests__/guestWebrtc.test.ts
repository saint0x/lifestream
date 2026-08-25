import { describe, expect, it } from "vitest";
import { looksLikeIceCandidate, looksLikeSdp } from "../guestWebrtc";

describe("guest WebRTC engine", () => {
  it("accepts bounded SDP offers and answers", () => {
    expect(looksLikeSdp("v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\n")).toBe(true);
    expect(looksLikeSdp("m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n")).toBe(false);
    expect(looksLikeSdp("v=0\r\n")).toBe(false);
  });

  it("recognizes browser ICE candidate payloads", () => {
    expect(looksLikeIceCandidate("candidate:0 1 UDP 2122252543 192.0.2.1 54400 typ host")).toBe(true);
    expect(looksLikeIceCandidate("not-a-candidate")).toBe(false);
  });
});
