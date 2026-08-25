import { useEffect, useMemo, useState } from "react";
import { Clapperboard, MonitorUp } from "lucide-react";
import {
  addBlockedTerm,
  addModerator,
  attachSponsorCampaign,
  captureSponsorProof,
  captureMediaPreviewFrame,
  captureMediaSegment,
  closeEngagementPoll,
  configureGuestRoomRouting,
  createGuestWebrtcOffer,
  createSourceFilter,
  createSceneFromTemplate,
  createSceneGroup,
  createEngagementAlert,
  createEngagementPoll,
  createScheduleSlot,
  createSponsorInventory,
  deleteScene,
  disableSourceFilter,
  duplicateScene,
  endBroadcast,
  emergencyDisconnect,
  exportObsCollection,
  discardRecording,
  createSupportBundle,
  getDashboard,
  getMediaCapabilities,
  getMediaCaptureInventory,
  getNativeHelperPackages,
  ingestRuntimeProgramFrame,
  ingestMediaSourceAudio,
  heartbeatNativeHelper,
  importObsCollection,
  enqueueModeration,
  inviteGuest,
  liveOpsOverride,
  negotiateGuestReturnFeed,
  packageMediaEncode,
  moderateGuest,
  patchScheduleSlot,
  patchBroadcast,
  patchAudioChannel,
  patchGuest,
  patchHotkey,
  patchSceneGroup,
  patchSource,
  patchSourceFilter,
  pauseRecording,
  pinMessage,
  previewTransition,
  renderMediaEncode,
  removeGuest,
  reorderScenes,
  recoverNativeHelper,
  recordInboundRaid,
  reconcileMediaCapture,
  reconcileGuestMediaRelays,
  reportAudienceTelemetry,
  reportGuestMediaTelemetry,
  reportNativeHelperCrash,
  reviewSponsorProof,
  resolveModeration,
  runtimeStreamUrl,
  runPreflight,
  runGuestDeviceCheck,
  saveReplay,
  scheduleRaidRedirect,
  sendSceneToProgram,
  shutdownNativeHelper,
  startBroadcast,
  startGuestIsolatedRecording,
  startMediaCapture,
  startMediaEncode,
  startNativeHelper,
  startRecording,
  resumeRecording,
  stopRecording,
  stopGuestIsolatedRecording,
  stopMediaCapture,
  stopMediaEncode,
  syncLocalObs,
  triggerHotkey,
  triggerCue,
  unpinMessage,
  voteEngagementPoll,
} from "@/app/api";
import { ProgramCanvas } from "@/components/studio/Canvas";
import { CompatibilityPanel } from "@/components/studio/CompatibilityPanel";
import { DevicePanel } from "@/components/studio/DevicePanel";
import { MediaPanel } from "@/components/studio/MediaPanel";
import { NativePanel } from "@/components/studio/NativePanel";
import {
  AudioMixer,
  AudiencePanel,
  ChannelPanel,
  CuePanel,
  EngagementPanel,
  GuestsPanel,
  HealthPanel,
  HotkeysPanel,
  Inspector,
  ModerationPanel,
  RuntimePanel,
  SafetyPanel,
  SceneGroupsPanel,
  SponsorPanel,
  TransitionPanel,
} from "@/components/studio/Panels";
import { Panel } from "@/components/studio/Panel";
import { SceneList, SourceList } from "@/components/studio/Rails";
import { TopBar } from "@/components/studio/TopBar";
import { createGuestWebrtcOfferPayload } from "@/engine/guestWebrtc";
import { matchingHotkey } from "@/engine/hotkeys";
import { dashboardFromRuntimeMessage, type RuntimeSocketState } from "@/engine/runtime";
import { movedSceneIds, type SceneMoveDirection } from "@/engine/scenes";
import { useDeviceSessions } from "@/engine/useDevices";
import type {
  NativeHelperPackage,
  NativeHelperSession,
  MediaCapabilities,
  MediaCaptureArtifact,
  MediaCaptureFrame,
  MediaCaptureInventory,
  MediaCaptureSession,
  MediaEncodeJob,
  MediaPackage,
  MediaSourceArtifact,
  ObsBridgeConnection,
  ObsDashboard,
  ObsExportJob,
  ObsImportReport,
  ObsRow,
} from "@/types";
import { text } from "@/types";

export function StudioPage() {
  const [data, setData] = useState<ObsDashboard | null>(null);
  const [selectedSceneId, setSelectedSceneId] = useState("scene_product_demo");
  const [selectedSourceId, setSelectedSourceId] = useState("source_camera_a");
  const [importReport, setImportReport] = useState<ObsImportReport | null>(null);
  const [exportJob, setExportJob] = useState<ObsExportJob | null>(null);
  const [bridgeConnection, setBridgeConnection] = useState<ObsBridgeConnection | null>(null);
  const [nativeSessions, setNativeSessions] = useState<readonly NativeHelperSession[]>([]);
  const [nativePackages, setNativePackages] = useState<readonly NativeHelperPackage[]>([]);
  const [mediaCapabilities, setMediaCapabilities] = useState<MediaCapabilities | null>(null);
  const [mediaCaptureInventory, setMediaCaptureInventory] = useState<MediaCaptureInventory | null>(null);
  const [captureSessions, setCaptureSessions] = useState<readonly MediaCaptureSession[]>([]);
  const [captureFrames, setCaptureFrames] = useState<readonly MediaCaptureFrame[]>([]);
  const [captureArtifacts, setCaptureArtifacts] = useState<readonly MediaCaptureArtifact[]>([]);
  const [sourceArtifacts, setSourceArtifacts] = useState<readonly MediaSourceArtifact[]>([]);
  const [encodeJobs, setEncodeJobs] = useState<readonly MediaEncodeJob[]>([]);
  const [packages, setPackages] = useState<readonly MediaPackage[]>([]);
  const [transitionPreview, setTransitionPreview] = useState<ObsRow | null>(null);
  const [runtimeSocketState, setRuntimeSocketState] = useState<RuntimeSocketState>("connecting");
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const devices = useDeviceSessions();

  const refresh = async () => {
    const dashboard = await getDashboard();
    setData(dashboard);
    setSelectedSceneId((current) => current || text(dashboard.runtime, "preview_scene_id"));
    setSelectedSourceId((current) => current || dashboard.sources[0]?.id || "");
  };

  useEffect(() => {
    void refresh().catch((err) => {
      setError(err instanceof Error ? err.message : "Unable to load Vanta Live Studio.");
    });
    void getMediaCapabilities().then(setMediaCapabilities).catch(() => {
      setMediaCapabilities(null);
    });
    void getMediaCaptureInventory().then(setMediaCaptureInventory).catch(() => {
      setMediaCaptureInventory(null);
    });
    void getNativeHelperPackages().then(setNativePackages).catch(() => {
      setNativePackages([]);
    });
  }, []);

  const selectedScene = useMemo(
    () => data?.scenes.find((scene) => scene.id === selectedSceneId) ?? data?.scenes[0] ?? null,
    [data, selectedSceneId],
  );
  const programScene = useMemo(
    () => data?.scenes.find((scene) => scene.id === text(data.runtime, "program_scene_id")) ?? selectedScene,
    [data, selectedScene],
  );
  const selectedSource = data?.sources.find((source) => source.id === selectedSourceId) ?? null;
  const previewInstances = useMemo(
    () => data?.instances.filter((instance) => text(instance, "scene_id") === selectedScene?.id) ?? [],
    [data, selectedScene],
  );
  const programInstances = useMemo(
    () => data?.instances.filter((instance) => text(instance, "scene_id") === programScene?.id) ?? [],
    [data, programScene],
  );
  const programCanvasCaptureSession = useMemo(
    () => captureSessions.find((session) => (
      text(session, "status") === "capturing" && text(session, "capture_kind") === "program_canvas"
    )) ?? null,
    [captureSessions],
  );

  const runAction = async (label: string, action: () => Promise<ObsDashboard | ObsRow>) => {
    setStatus(label);
    setError(null);
    try {
      const result = await action();
      if ("broadcast" in result) setData(result as ObsDashboard);
      await refresh();
      setStatus(`${label} complete`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Action failed.");
    }
  };

  const runImport = async (file: File) => {
    await runAction("Importing OBS", async () => {
      const collectionJson = JSON.parse(await file.text()) as unknown;
      const report = await importObsCollection(collectionJson);
      setImportReport(report);
      return report;
    });
  };

  const runExport = async () => {
    if (!data) return;
    await runAction("Exporting OBS", async () => {
      const job = await exportObsCollection(data.collection.id, `${text(data.collection, "name")} OBS Export`);
      setExportJob(job);
      return job;
    });
  };

  const runSync = async () => {
    await runAction("Syncing OBS", async () => {
      const connection = await syncLocalObs();
      setBridgeConnection(connection);
      return connection;
    });
  };

  const runSceneMove = async (sceneId: string, direction: SceneMoveDirection) => {
    if (!data) return;
    const sceneIds = movedSceneIds(data.scenes, sceneId, direction);
    await runAction("Reordering scenes", () => reorderScenes(data.collection.id, sceneIds));
  };

  const runSceneDelete = async (sceneId: string) => {
    await runAction("Deleting scene", async () => {
      const dashboard = await deleteScene(sceneId);
      setSelectedSceneId((current) => {
        if (current !== sceneId) return current;
        return dashboard.scenes[0]?.id ?? "";
      });
      return dashboard;
    });
  };

  const runNativeStart = async (kind: string) => {
    await runAction(`Starting ${kind} helper`, async () => {
      const session = await startNativeHelper(kind);
      setNativeSessions((current) => [session, ...current.filter((item) => item.id !== session.id)]);
      return session;
    });
  };

  const runNativeHeartbeat = async (sessionId: string) => {
    await runAction("Checking native helper", async () => {
      const session = await heartbeatNativeHelper(sessionId);
      setNativeSessions((current) => current.map((item) => item.id === session.id ? session : item));
      return session;
    });
  };

  const runNativeRecover = async (sessionId: string) => {
    await runAction("Recovering native helper", async () => {
      const session = await recoverNativeHelper(sessionId);
      setNativeSessions((current) => current.map((item) => item.id === session.id ? session : item));
      return session;
    });
  };

  const runNativeCrashReport = async (sessionId: string) => {
    await runAction("Reporting native helper crash", async () => {
      const session = await reportNativeHelperCrash(sessionId);
      setNativeSessions((current) => current.map((item) => item.id === session.id ? session : item));
      return session;
    });
  };

  const runNativeShutdown = async (sessionId: string) => {
    await runAction("Stopping native helper", async () => {
      const session = await shutdownNativeHelper(sessionId);
      setNativeSessions((current) => current.map((item) => item.id === session.id ? session : item));
      return session;
    });
  };

  const runCaptureStart = async () => {
    if (!selectedSource) return;
    await runAction("Preparing capture", async () => {
      const session = await startMediaCapture(selectedSource);
      setCaptureSessions((current) => [session, ...current.filter((item) => item.id !== session.id)]);
      return session;
    });
  };

  const runCaptureStop = async (sessionId: string) => {
    await runAction("Stopping capture", async () => {
      const session = await stopMediaCapture(sessionId);
      setCaptureSessions((current) => current.map((item) => item.id === session.id ? session : item));
      return session;
    });
  };

  const runCaptureReconcile = async (sessionId: string) => {
    await runAction("Reconciling capture", async () => {
      const session = await reconcileMediaCapture(sessionId);
      setCaptureSessions((current) => current.map((item) => item.id === session.id ? session : item));
      const inventory = await getMediaCaptureInventory();
      setMediaCaptureInventory(inventory);
      return session;
    });
  };

  const runCapturePreviewFrame = async (sessionId: string) => {
    await runAction("Capturing preview frame", async () => {
      const frame = await captureMediaPreviewFrame(sessionId);
      setCaptureFrames((current) => [frame, ...current.filter((item) => item.id !== frame.id)]);
      return frame;
    });
  };

  const runRuntimeProgramFrame = async (
    sessionId: string,
    imageDataUrl: string,
    compositorBackend: "webgl_gpu" | "canvas_2d",
    frameSequence: number,
  ) => {
    const frame = await ingestRuntimeProgramFrame(sessionId, imageDataUrl, compositorBackend, frameSequence);
    setCaptureFrames((current) => [frame, ...current.filter((item) => item.id !== frame.id)]);
  };

  const runCaptureSegment = async (sessionId: string) => {
    await runAction("Capturing display segment", async () => {
      const artifact = await captureMediaSegment(sessionId);
      setCaptureArtifacts((current) => [artifact, ...current.filter((item) => item.id !== artifact.id)]);
      return artifact;
    });
  };

  const runSourceAudioIngest = async () => {
    if (!selectedSource) return;
    const inputPath = sourceMediaPath(selectedSource);
    if (!inputPath) return;
    await runAction("Ingesting source audio", async () => {
      const artifact = await ingestMediaSourceAudio(selectedSource.id, inputPath);
      setSourceArtifacts((current) => [artifact, ...current.filter((item) => item.id !== artifact.id)]);
      return artifact;
    });
  };

  const runEncodeStart = async (captureSessionId: string) => {
    await runAction("Preparing encode", async () => {
      const job = await startMediaEncode(broadcastId, captureSessionId);
      setEncodeJobs((current) => [job, ...current.filter((item) => item.id !== job.id)]);
      return job;
    });
  };

  const runEncodeStop = async (jobId: string) => {
    await runAction("Finalizing encode", async () => {
      const job = await stopMediaEncode(jobId);
      setEncodeJobs((current) => current.map((item) => item.id === job.id ? job : item));
      return job;
    });
  };

  const runEncodeRender = async (jobId: string) => {
    await runAction("Rendering output", async () => {
      const job = await renderMediaEncode(jobId);
      setEncodeJobs((current) => current.map((item) => item.id === job.id ? job : item));
      return job;
    });
  };

  const runEncodePackage = async (jobId: string) => {
    await runAction("Packaging output", async () => {
      const pkg = await packageMediaEncode(jobId);
      setPackages((current) => [pkg, ...current.filter((item) => item.id !== pkg.id)]);
      return pkg;
    });
  };

  const runAudioPatch = async (channel: ObsRow, patch: Record<string, unknown>) => {
    await runAction("Updating audio", async () => {
      await patchAudioChannel(channel.id, patch);
      const dashboard = await getDashboard();
      setData(dashboard);
      return dashboard;
    });
  };

  const runGuestWebrtcOffer = async (participantId: string) => {
    await runAction("Preparing guest WebRTC", async () => {
      const payload = await createGuestWebrtcOfferPayload({
        participantId,
        audio: true,
        video: true,
        preferredVideoLayer: "720p30",
      });
      return createGuestWebrtcOffer(participantId, payload);
    });
  };

  const runTransitionPreview = async () => {
    if (!selectedScene || !data) return;
    setStatus("Previewing transition");
    setError(null);
    try {
      const preview = await previewTransition(selectedScene.id, text(data.runtime, "program_scene_id"));
      setTransitionPreview(preview);
      setStatus("Previewing transition complete");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Transition preview failed.");
    }
  };

  useEffect(() => {
    if (!data) return;
    const onKeyDown = (event: KeyboardEvent) => {
      const hotkey = matchingHotkey(data.hotkeys, event);
      if (!hotkey) return;
      event.preventDefault();
      void runAction(`Hotkey ${text(hotkey, "binding")}`, () => triggerHotkey(hotkey.id));
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [data]);

  useEffect(() => {
    if (!data) return;
    let closedByEffect = false;
    let reconnectTimer: number | undefined;
    const connect = () => {
      setRuntimeSocketState((current) => current === "connecting" ? current : "reconnecting");
      const socket = new WebSocket(runtimeStreamUrl(data.broadcast.id));
      socket.onopen = () => setRuntimeSocketState("live");
      socket.onmessage = (event) => {
        if (typeof event.data !== "string") return;
        const dashboard = dashboardFromRuntimeMessage(event.data);
        if (dashboard) setData(dashboard);
      };
      socket.onerror = () => setRuntimeSocketState("offline");
      socket.onclose = () => {
        if (closedByEffect) return;
        setRuntimeSocketState("reconnecting");
        reconnectTimer = window.setTimeout(connect, 1500);
      };
      return socket;
    };
    const socket = connect();
    return () => {
      closedByEffect = true;
      if (reconnectTimer) window.clearTimeout(reconnectTimer);
      socket.close();
    };
  }, [data?.broadcast.id]);

  if (!data) {
    return <div className="obs-route-state mono">{error ?? "Loading Vanta Live Studio..."}</div>;
  }

  const broadcastId = data.broadcast.id;

  return (
    <main className="obs">
      <TopBar
        data={data}
        status={status}
        onStart={() => runAction("Starting stream", () => startBroadcast(broadcastId))}
        onEnd={() => runAction("Ending stream", () => endBroadcast(broadcastId))}
        onRecord={() => runAction("Starting recording", () => startRecording(broadcastId))}
        onPauseRecord={() => runAction("Pausing recording", () => pauseRecording(broadcastId).then(() => getDashboard()))}
        onResumeRecord={() => runAction("Resuming recording", () => resumeRecording(broadcastId).then(() => getDashboard()))}
        onStopRecord={() => runAction("Stopping recording", () => stopRecording(broadcastId).then(() => getDashboard()))}
        onReplay={(options) => runAction("Saving replay", () => saveReplay(broadcastId, options))}
      />
      {error ? <div className="obs-notice obs-notice--error">{error}</div> : null}
      <section className="obs-workspace">
        <aside className="obs-rail obs-rail--left">
          <Panel
            title="Scenes"
            icon={<Clapperboard />}
            summary={<strong>{data.scenes.length}</strong>}
            defaultCollapsed
          >
            <SceneList
              scenes={data.scenes}
              templates={data.scene_templates}
              activeId={text(data.runtime, "program_scene_id")}
              selectedId={selectedScene?.id ?? ""}
              onSelect={setSelectedSceneId}
              onDuplicate={(sceneId) => runAction("Duplicating scene", () => duplicateScene(sceneId))}
              onMove={runSceneMove}
              onDelete={runSceneDelete}
              onCreateFromTemplate={(templateId) => (
                runAction("Adding scene template", () => createSceneFromTemplate(templateId, data.collection.id))
              )}
            />
          </Panel>
          <Panel
            title="Sources"
            icon={<MonitorUp />}
            summary={<strong>{data.sources.length}</strong>}
            defaultCollapsed
          >
            <SourceList sources={data.sources} selectedId={selectedSourceId} onSelect={setSelectedSourceId} />
          </Panel>
          <Inspector
            source={selectedSource}
            onPatchSource={(sourceId, patch) => (
              runAction("Updating source", async () => {
                const dashboard = await patchSource(sourceId, patch);
                setData(dashboard);
                return dashboard;
              })
            )}
            onCreateFilter={(sourceId) => (
              runAction("Adding filter", async () => {
                await createSourceFilter(sourceId);
                const dashboard = await getDashboard();
                setData(dashboard);
                return dashboard;
              })
            )}
            onPatchFilter={(filterId, patch) => (
              runAction("Updating filter", async () => {
                await patchSourceFilter(filterId, patch);
                const dashboard = await getDashboard();
                setData(dashboard);
                return dashboard;
              })
            )}
            onDisableFilter={(filterId) => (
              runAction("Disabling filter", async () => {
                await disableSourceFilter(filterId);
                const dashboard = await getDashboard();
                setData(dashboard);
                return dashboard;
              })
            )}
          />
          <SceneGroupsPanel
            scene={selectedScene}
            scenes={data.scenes}
            sources={data.sources}
            instances={data.instances}
            onCreate={(childSceneId) => (
              selectedScene && runAction("Adding scene group", () => createSceneGroup(selectedScene.id, childSceneId))
            )}
            onPatch={(sourceId, childSceneId) => (
              runAction("Updating scene group", () => patchSceneGroup(sourceId, { child_scene_id: childSceneId }))
            )}
          />
          <AudiencePanel
            audience={data.audience}
            onSample={() => runAction("Sampling audience", () => reportAudienceTelemetry(broadcastId, {
              viewerCount: 1284,
              chatMessagesPerMinute: 138,
              tipsCents: 799,
              subscriptions: 3,
              revenueCents: 2299,
              discoverySource: "category_rank",
              discoveryScore: 88.2,
            }))}
            onRaid={() => runAction("Scheduling raid", () => scheduleRaidRedirect(broadcastId))}
            onInboundRaid={() => runAction("Recording inbound raid", () => recordInboundRaid(broadcastId))}
          />
          <EngagementPanel
            engagement={data.engagement}
            onSchedule={() => runAction("Scheduling live", () => createScheduleSlot(broadcastId))}
            onReschedule={(slotId) => runAction("Rescheduling live", () => patchScheduleSlot(slotId, {
              status: "rescheduled",
              duration_minutes: 60,
            }))}
            onPoll={() => runAction("Opening poll", () => createEngagementPoll(broadcastId, "poll"))}
            onPrediction={() => runAction("Opening prediction", () => createEngagementPoll(broadcastId, "prediction"))}
            onVote={(pollId, optionId) => runAction("Voting poll", () => voteEngagementPoll(pollId, optionId))}
            onClosePoll={(pollId) => runAction("Closing poll", () => closeEngagementPoll(pollId))}
            onAlert={() => runAction("Queuing alert", () => createEngagementAlert(broadcastId))}
          />
          <SponsorPanel
            sponsor={data.sponsor}
            onAttach={() => runAction("Attaching sponsor", () => attachSponsorCampaign(broadcastId))}
            onInventory={(creativeKind) => (
              runAction("Scheduling sponsor", () => createSponsorInventory(broadcastId, creativeKind))
            )}
            onProof={(inventoryId) => runAction("Capturing proof", () => captureSponsorProof(inventoryId))}
            onReview={(proofId) => runAction("Reviewing proof", () => reviewSponsorProof(proofId))}
          />
          <AudioMixer channels={data.audio} onPatch={runAudioPatch} />
          <HotkeysPanel
            hotkeys={data.hotkeys}
            onTrigger={(hotkeyId) => runAction("Running hotkey", () => triggerHotkey(hotkeyId))}
            onToggle={(hotkeyId, enabled) => (
              runAction("Updating hotkey", async () => {
                await patchHotkey(hotkeyId, { enabled });
                const dashboard = await getDashboard();
                setData(dashboard);
                return dashboard;
              })
            )}
          />
          <CuePanel
            cues={data.cues}
            onTrigger={(cueId) => runAction("Triggering cue", () => triggerCue(cueId))}
          />
        </aside>

        <section className="obs-stage">
          <ProgramCanvas
            title="Program"
            scene={programScene}
            instances={programInstances}
            allInstances={data.instances}
            sources={data.sources}
            streams={devices.sessions}
            live={text(data.runtime, "stream_state") === "live"}
            runtimeFrameSessionId={programCanvasCaptureSession?.id ?? null}
            onRuntimeFrame={runRuntimeProgramFrame}
          />
          <div className="obs-stage__lower">
            <ProgramCanvas
              title="Preview"
              scene={selectedScene}
              instances={previewInstances}
              allInstances={data.instances}
              sources={data.sources}
              streams={devices.sessions}
              compact
            />
            <TransitionPanel
              scene={selectedScene}
              runtime={data.runtime}
              preview={transitionPreview}
              onSend={() => selectedScene && runAction("Sending scene to program", () => sendSceneToProgram(selectedScene.id))}
              onPreview={runTransitionPreview}
              onPreflight={() => runAction("Running preflight", () => runPreflight(broadcastId, data.collection.id))}
            />
          </div>
        </section>

        <aside className="obs-rail obs-rail--right">
          <ChannelPanel
            broadcast={data.broadcast}
            runtime={data.runtime}
            onPatch={(patch) => runAction("Updating channel", () => patchBroadcast(broadcastId, patch))}
          />
          <GuestsPanel
            guests={data.guests}
            targetSceneId={selectedScene?.id ?? text(data.runtime, "preview_scene_id")}
            onInvite={() => runAction("Inviting guest", () => inviteGuest(broadcastId))}
            onRouting={(mode) => (
              runAction("Updating guest routing", () => configureGuestRoomRouting(broadcastId, mode))
            )}
            onRelay={() => runAction("Reconciling guest relays", () => reconcileGuestMediaRelays(broadcastId))}
            onDeviceCheck={(participantId) => (
              runAction("Checking guest devices", () => runGuestDeviceCheck(participantId))
            )}
            onMediaTelemetry={(participantId) => (
              runAction("Updating active speaker", () => reportGuestMediaTelemetry(participantId))
            )}
            onWebrtcOffer={runGuestWebrtcOffer}
            onReturnFeed={(participantId) => (
              runAction("Negotiating return feed", () => (
                negotiateGuestReturnFeed(
                  participantId,
                  text(data.guests, "room_mode") === "shared_game" ? "source_screen" : undefined,
                )
              ))
            )}
            onIsolatedRecording={(participantId, recording) => (
              runAction(recording ? "Stopping isolated recording" : "Starting isolated recording", () => (
                recording ? stopGuestIsolatedRecording(participantId) : startGuestIsolatedRecording(participantId)
              ))
            )}
            onModerate={(participantId, action) => (
              runAction("Moderating guest", () => (
                moderateGuest(
                  participantId,
                  action,
                  action === "approve_live" ? selectedScene?.id ?? text(data.runtime, "preview_scene_id") : undefined,
                )
              ))
            )}
            onPatch={(participantId, patch) => (
              runAction("Updating guest", () => patchGuest(participantId, patch))
            )}
            onRemove={(participantId) => runAction("Removing guest", () => removeGuest(participantId))}
          />
          <SafetyPanel
            safety={data.safety}
            onEmergencyDisconnect={() => runAction("Emergency hold", () => emergencyDisconnect(broadcastId))}
            onLiveOpsOverride={(action) => (
              runAction("Live Ops override", () => (
                liveOpsOverride(broadcastId, action, `Live Ops ${action.replace("_", " ")}`)
              ))
            )}
            onSupportBundle={() => runAction("Exporting support bundle", () => createSupportBundle(broadcastId))}
          />
          <DevicePanel
            catalog={devices.catalog}
            sessions={devices.sessions}
            onRequest={(kind) => {
              setStatus(`Arming ${kind}`);
              void devices.request(kind).finally(() => setStatus(`${kind} armed`));
            }}
            onStop={(kind) => {
              devices.stop(kind);
              setStatus(`${kind} stopped`);
            }}
            onReconnect={(kind) => {
              setStatus(`Reconnecting ${kind}`);
              void devices.reconnect(kind).finally(() => setStatus(`${kind} reconnect checked`));
            }}
          />
          <NativePanel
            sessions={nativeSessions}
            packages={nativePackages}
            busy={status !== null && !status.endsWith("complete")}
            onStart={runNativeStart}
            onHeartbeat={runNativeHeartbeat}
            onRecover={runNativeRecover}
            onCrashReport={runNativeCrashReport}
            onShutdown={runNativeShutdown}
          />
          <MediaPanel
            selectedSource={selectedSource}
            capabilities={mediaCapabilities}
            inventory={mediaCaptureInventory}
            captureSessions={captureSessions}
            captureFrames={captureFrames}
            captureArtifacts={captureArtifacts}
            sourceArtifacts={sourceArtifacts}
            encodeJobs={encodeJobs}
            packages={packages}
            busy={status !== null && !status.endsWith("complete")}
            onStartCapture={runCaptureStart}
            onStopCapture={runCaptureStop}
            onCaptureReconcile={runCaptureReconcile}
            onCapturePreviewFrame={runCapturePreviewFrame}
            onCaptureSegment={runCaptureSegment}
            onSourceAudioIngest={runSourceAudioIngest}
            onStartEncode={runEncodeStart}
            onStopEncode={runEncodeStop}
            onRenderEncode={runEncodeRender}
            onPackageEncode={runEncodePackage}
          />
          <ModerationPanel
            moderation={data.moderation}
            onBlockTerm={() => runAction("Adding blocked term", () => addBlockedTerm(broadcastId))}
            onModerator={() => runAction("Adding moderator", () => addModerator(broadcastId))}
            onQueue={() => runAction("Queueing moderation", () => enqueueModeration(broadcastId))}
            onResolve={(itemId, resolveStatus) => (
              runAction("Resolving moderation", () => resolveModeration(itemId, resolveStatus))
            )}
            onPin={() => runAction("Pinning message", () => pinMessage(broadcastId))}
            onUnpin={(messageId) => runAction("Unpinning message", () => unpinMessage(messageId))}
          />
          <HealthPanel health={data.health} preflight={data.preflight} />
          <CompatibilityPanel
            importReport={importReport}
            exportJob={exportJob}
            bridgeConnection={bridgeConnection}
            busy={status !== null && !status.endsWith("complete")}
            onImportFile={runImport}
            onExport={runExport}
            onSync={runSync}
          />
          <RuntimePanel
            events={data.events}
            replays={data.replays}
            runtime={data.runtime}
            health={data.health}
            postShow={data.post_show}
            streamState={runtimeSocketState}
            onDiscardRecording={() => runAction("Discarding recording", () => discardRecording(broadcastId).then(() => getDashboard()))}
          />
        </aside>
      </section>
    </main>
  );
}

function sourceMediaPath(source: ObsRow): string {
  const settings = source.default_settings_json;
  if (settings && typeof settings === "object" && !Array.isArray(settings)) {
    const value = (settings as Record<string, unknown>).media_path;
    if (typeof value === "string" && value.trim()) return value;
  }
  const direct = source.media_path;
  return typeof direct === "string" ? direct : "";
}
