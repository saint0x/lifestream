import { useMemo, useState } from "react";
import { Activity, BadgeCheck, Circle, Disc3, Gauge, Pause, Play, Radio, Save, Square } from "lucide-react";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import {
  REPLAY_DURATION_PRESETS,
  clampReplayDuration,
  replayDurationFromPreset,
  type ReplayDraftOptions,
  type ReplayPreset,
} from "@/engine/replay";
import { formatNumber } from "@/lib/format";
import type { ObsDashboard } from "@/types";
import { num, text } from "@/types";

export function TopBar({
  data,
  status,
  onStart,
  onEnd,
  onRecord,
  onPauseRecord,
  onResumeRecord,
  onStopRecord,
  onReplay,
}: {
  readonly data: ObsDashboard;
  readonly status: string | null;
  readonly onStart: () => void;
  readonly onEnd: () => void;
  readonly onRecord: () => void;
  readonly onPauseRecord: () => void;
  readonly onResumeRecord: () => void;
  readonly onStopRecord: () => void;
  readonly onReplay: (options: ReplayDraftOptions) => void;
}) {
  const streamState = text(data.runtime, "stream_state");
  const recordingState = text(data.runtime, "recording_state");
  const playbackReady = runtimeNestedText(data.runtime, "playback_readiness_json", "status") === "ready";
  const [replayPreset, setReplayPreset] = useState<ReplayPreset>(30);
  const [customReplayDuration, setCustomReplayDuration] = useState(45);
  const [sponsorProof, setSponsorProof] = useState(false);
  const [armedAction, setArmedAction] = useState<"end" | "stop-recording" | null>(null);
  const replayDuration = useMemo(
    () => replayDurationFromPreset(replayPreset, customReplayDuration),
    [customReplayDuration, replayPreset],
  );

  const saveReplayDraft = () => {
    onReplay({
      durationSeconds: replayDuration,
      sponsorProof,
    });
  };

  return (
    <header className="obs-top">
      <div className="obs-top__identity">
        <Badge tone={streamState === "live" ? "live" : "new"}>{streamState}</Badge>
        <div>
          <strong>{text(data.broadcast, "title")}</strong>
          <span className="mono">
            {text(data.collection, "name")} / {text(data.broadcast, "output_quality_target")}
          </span>
        </div>
      </div>
      <div className="obs-top__meters mono">
        <span><Activity size={14} /> {text(data.health, "status")}</span>
        <span><Gauge size={14} /> {formatNumber(num(data.health, "bitrate_kbps"))} kbps</span>
        <span><Radio size={14} /> {runtimeNestedText(data.runtime, "runtime_target_json", "protocol", "pending")}</span>
        <Badge tone={playbackReady ? "hd" : "premium"}>{playbackReady ? "playback ready" : "playback pending"}</Badge>
        <span><Disc3 size={14} /> {recordingState}</span>
        {status ? <span>{status}</span> : null}
      </div>
      <div className="obs-top__actions">
        <div className="obs-top__replay">
          <div className="obs-top__segments" role="group" aria-label="Replay duration">
            {REPLAY_DURATION_PRESETS.map((duration) => (
              <Button
                key={duration}
                size="sm"
                variant={replayPreset === duration ? "primary" : "ghost"}
                aria-pressed={replayPreset === duration}
                onClick={() => setReplayPreset(duration)}
              >
                {duration}s
              </Button>
            ))}
            <Button
              size="sm"
              variant={replayPreset === "custom" ? "primary" : "ghost"}
              aria-pressed={replayPreset === "custom"}
              onClick={() => setReplayPreset("custom")}
            >
              Custom
            </Button>
          </div>
          {replayPreset === "custom" ? (
            <Input
              className="obs-top__duration"
              type="number"
              min={5}
              max={300}
              step={1}
              value={customReplayDuration}
              aria-label="Custom replay seconds"
              onChange={(event) => setCustomReplayDuration(clampReplayDuration(event.currentTarget.valueAsNumber))}
            />
          ) : null}
          <Button
            size="sm"
            variant={sponsorProof ? "primary" : "secondary"}
            icon={<BadgeCheck />}
            aria-pressed={sponsorProof}
            onClick={() => setSponsorProof((current) => !current)}
          >
            Proof
          </Button>
          <Button size="sm" variant="secondary" icon={<Save />} onClick={saveReplayDraft}>
            {replayDuration}s
          </Button>
        </div>
        {recordingState === "recording" || recordingState === "paused" ? (
          <>
            <Button
              size="sm"
              variant="secondary"
              icon={recordingState === "paused" ? <Play /> : <Pause />}
              onClick={recordingState === "paused" ? onResumeRecord : onPauseRecord}
            >
              {recordingState === "paused" ? "Resume" : "Pause"}
            </Button>
            <Button
              size="sm"
              variant={armedAction === "stop-recording" ? "danger" : "secondary"}
              icon={<Square />}
              onClick={() => {
                if (armedAction === "stop-recording") {
                  setArmedAction(null);
                  onStopRecord();
                } else {
                  setArmedAction("stop-recording");
                }
              }}
            >
              {armedAction === "stop-recording" ? "STOP RECORDING" : "Stop Rec"}
            </Button>
          </>
        ) : (
          <Button
            size="sm"
            variant="secondary"
            icon={<Circle />}
            onClick={onRecord}
          >
            Record
          </Button>
        )}
        {streamState === "live" ? (
          <Button
            size="sm"
            variant="danger"
            icon={<Square />}
            onClick={() => {
              if (armedAction === "end") {
                setArmedAction(null);
                onEnd();
              } else {
                setArmedAction("end");
              }
            }}
          >
            {armedAction === "end" ? "END STREAM" : "End"}
          </Button>
        ) : (
          <Button size="sm" variant="primary" icon={<Radio />} onClick={onStart}>Go Live</Button>
        )}
      </div>
    </header>
  );
}

function runtimeNestedText(row: Record<string, unknown>, field: string, key: string, fallback = ""): string {
  const value = row[field];
  if (!value || typeof value !== "object" || Array.isArray(value)) return fallback;
  const nested = value as Record<string, unknown>;
  return typeof nested[key] === "string" ? nested[key] : fallback;
}
