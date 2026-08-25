import { useCallback, useEffect, useRef, useState } from "react";
import {
  EMPTY_CATALOG,
  EMPTY_SESSIONS,
  type CaptureKind,
  type CaptureSession,
  type DeviceCatalog,
  browserMediaDevices,
  disconnectedSession,
  enumerateCaptureDevices,
  reconcileCatalogSessions,
  requestCaptureStream,
  shouldReconnectSession,
  stopCaptureSession,
} from "./devices";

export function useDeviceSessions() {
  const [catalog, setCatalog] = useState<DeviceCatalog>(EMPTY_CATALOG);
  const [sessions, setSessions] = useState<Record<CaptureKind, CaptureSession>>(EMPTY_SESSIONS);
  const sessionsRef = useRef(sessions);

  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);

  const refreshCatalog = useCallback(async () => {
    const nextCatalog = await enumerateCaptureDevices();
    setCatalog(nextCatalog);
    setSessions((current) => reconcileCatalogSessions(nextCatalog, current));
    return nextCatalog;
  }, []);

  useEffect(() => {
    const mediaDevices = browserMediaDevices();
    void refreshCatalog();
    mediaDevices?.addEventListener?.("devicechange", refreshCatalog);
    return () => {
      mediaDevices?.removeEventListener?.("devicechange", refreshCatalog);
      Object.values(sessionsRef.current).forEach((session) => {
        session.stream?.getTracks().forEach((track) => track.stop());
      });
    };
  }, [refreshCatalog]);

  const markDisconnected = useCallback((kind: CaptureKind) => {
    setSessions((current) => ({
      ...current,
      [kind]: disconnectedSession(current[kind], `${current[kind].label} media track ended.`),
    }));
  }, []);

  const request = useCallback(async (kind: CaptureKind) => {
    setSessions((current) => ({
      ...current,
      [kind]: { ...current[kind], status: "checking", error: null },
    }));
    const session = await requestCaptureStream(kind);
    setSessions((current) => ({
      ...current,
      [kind]: session.status === "ready"
        ? attachTrackEnded(session, () => markDisconnected(kind))
        : session,
    }));
    await refreshCatalog();
  }, [markDisconnected, refreshCatalog]);

  const stop = useCallback((kind: CaptureKind) => {
    setSessions((current) => ({
      ...current,
      [kind]: stopCaptureSession(current[kind]),
    }));
  }, []);

  const reconnect = useCallback(async (kind: CaptureKind) => {
    const current = sessionsRef.current[kind];
    setSessions((sessions) => ({
      ...sessions,
      [kind]: {
        ...sessions[kind],
        status: "checking",
        error: "Reconnecting to the last known device.",
      },
    }));
    const session = await requestCaptureStream(
      kind,
      browserMediaDevices(),
      current.deviceId,
      current.reconnectAttempts + 1,
    );
    setSessions((sessions) => ({
      ...sessions,
      [kind]: session.status === "ready"
        ? attachTrackEnded(session, () => markDisconnected(kind))
        : session,
    }));
    await refreshCatalog();
  }, [markDisconnected, refreshCatalog]);

  useEffect(() => {
    const reconnectable = Object.values(sessions).filter(shouldReconnectSession);
    for (const session of reconnectable) {
      void reconnect(session.kind);
    }
  }, [reconnect, sessions]);

  return {
    catalog,
    sessions,
    refreshCatalog,
    request,
    reconnect,
    stop,
  };
}

function attachTrackEnded(session: CaptureSession, onEnded: () => void): CaptureSession {
  session.stream?.getTracks().forEach((track) => {
    track.addEventListener?.("ended", onEnded, { once: true });
  });
  return session;
}
