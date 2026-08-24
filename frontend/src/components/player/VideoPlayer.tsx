import { useCallback, useEffect, useRef, useState } from "react";
import {
  Play,
  Pause,
  Volume2,
  VolumeX,
  Maximize2,
  Minimize2,
  Subtitles,
  SkipForward,
  SkipBack,
} from "lucide-react";
import { formatDuration } from "@/lib/format";
import "./VideoPlayer.css";

interface VideoPlayerProps {
  readonly poster: string;
  readonly title: string;
  readonly subtitle?: string;
  readonly durationSec: number;
  readonly initialProgressSec?: number;
  readonly onProgress?: (sec: number) => void;
  readonly sourceUrl?: string | null;
  readonly allowPreviewTransport?: boolean;
}

export function VideoPlayer({
  poster,
  title,
  subtitle,
  durationSec,
  initialProgressSec = 0,
  onProgress,
  sourceUrl,
  allowPreviewTransport = true,
}: VideoPlayerProps) {
  const isTransportBacked = Boolean(sourceUrl);
  const videoRef = useRef<HTMLVideoElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const hideTimer = useRef<number | null>(null);
  const hasAppliedInitialSeek = useRef(false);
  const [playing, setPlaying] = useState(false);
  const [progress, setProgress] = useState(initialProgressSec);
  const [mediaDuration, setMediaDuration] = useState(durationSec);
  const [volume, setVolume] = useState(0.8);
  const [muted, setMuted] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [subsOn, setSubsOn] = useState(true);
  const [showControls, setShowControls] = useState(true);
  const [playbackError, setPlaybackError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTransportBacked) {
      hasAppliedInitialSeek.current = true;
      return;
    }

    let cancelled = false;
    let hls: {
      destroy: () => void;
      loadSource: (source: string) => void;
      attachMedia: (video: HTMLVideoElement) => void;
      on: (event: string, callback: (_event: string, data: { fatal: boolean }) => void) => void;
    } | null = null;
    const video = videoRef.current;
    if (!video || !sourceUrl) return;

    hasAppliedInitialSeek.current = false;
    setPlaybackError(null);
    video.poster = poster;

    const attachSource = async () => {
      if (video.canPlayType("application/vnd.apple.mpegurl")) {
        video.src = sourceUrl;
        return;
      }

      const module = await import("hls.js");
      const Hls = module.default as unknown as {
        new (config?: Record<string, unknown>): {
          destroy: () => void;
          loadSource: (source: string) => void;
          attachMedia: (video: HTMLVideoElement) => void;
          on: (event: string, callback: (_event: string, data: { fatal: boolean }) => void) => void;
        };
        isSupported: () => boolean;
        Events: { ERROR: string };
      };
      if (!Hls.isSupported()) {
        if (!cancelled) {
          setPlaybackError("This browser does not support HLS playback.");
        }
        return;
      }

      hls = new Hls({
        enableWorker: true,
        backBufferLength: 90,
      });
      const instance = hls;
      instance.loadSource(sourceUrl);
      instance.attachMedia(video);
      instance.on(Hls.Events.ERROR, (_event, data) => {
        if (!cancelled && data.fatal) {
          setPlaybackError("Playback failed while loading the stream.");
        }
      });
    };

    void attachSource();

    return () => {
      cancelled = true;
      hls?.destroy();
      video.removeAttribute("src");
      video.load();
    };
  }, [isTransportBacked, poster, sourceUrl]);

  useEffect(() => {
    const video = videoRef.current;
    if (!isTransportBacked || !video) return;

    const onLoadedMetadata = () => {
      if (!hasAppliedInitialSeek.current) {
        video.currentTime = Math.min(initialProgressSec, video.duration || initialProgressSec);
        hasAppliedInitialSeek.current = true;
      }
      setMediaDuration(Number.isFinite(video.duration) && video.duration > 0 ? video.duration : durationSec);
      setProgress(video.currentTime || initialProgressSec);
    };
    const onTimeUpdate = () => {
      const current = video.currentTime || 0;
      setProgress(current);
      onProgress?.(Math.floor(current));
    };
    const onPlay = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    const onEnded = () => setPlaying(false);
    const onDurationChange = () => {
      setMediaDuration(Number.isFinite(video.duration) && video.duration > 0 ? video.duration : durationSec);
    };
    const onVolumeChange = () => {
      setVolume(video.volume);
      setMuted(video.muted);
    };
    const onError = () => setPlaybackError("The video stream could not be played.");

    video.addEventListener("loadedmetadata", onLoadedMetadata);
    video.addEventListener("timeupdate", onTimeUpdate);
    video.addEventListener("play", onPlay);
    video.addEventListener("pause", onPause);
    video.addEventListener("ended", onEnded);
    video.addEventListener("durationchange", onDurationChange);
    video.addEventListener("volumechange", onVolumeChange);
    video.addEventListener("error", onError);

    return () => {
      video.removeEventListener("loadedmetadata", onLoadedMetadata);
      video.removeEventListener("timeupdate", onTimeUpdate);
      video.removeEventListener("play", onPlay);
      video.removeEventListener("pause", onPause);
      video.removeEventListener("ended", onEnded);
      video.removeEventListener("durationchange", onDurationChange);
      video.removeEventListener("volumechange", onVolumeChange);
      video.removeEventListener("error", onError);
    };
  }, [durationSec, initialProgressSec, isTransportBacked, onProgress]);

  useEffect(() => {
    if (isTransportBacked) return;
    if (!playing) return;
    const id = window.setInterval(() => {
      setProgress((current) => {
        const next = Math.min(current + 1, durationSec);
        onProgress?.(next);
        if (next >= durationSec) setPlaying(false);
        return next;
      });
    }, 1000);
    return () => window.clearInterval(id);
  }, [durationSec, isTransportBacked, onProgress, playing]);

  const resolvedDuration = Math.max(mediaDuration || durationSec, 1);
  const pct = (progress / resolvedDuration) * 100;

  const scheduleHide = () => {
    if (hideTimer.current) window.clearTimeout(hideTimer.current);
    setShowControls(true);
    hideTimer.current = window.setTimeout(() => {
      if (playing) setShowControls(false);
    }, 2600);
  };

  const syncFallbackProgress = useCallback((next: number) => {
    const bounded = Math.min(Math.max(next, 0), resolvedDuration);
    setProgress(bounded);
    onProgress?.(Math.floor(bounded));
  }, [onProgress, resolvedDuration]);

  const togglePlay = useCallback(async () => {
    const video = videoRef.current;
    if ((!isTransportBacked && allowPreviewTransport) || !video) {
      setPlaying((current) => !current);
      return;
    }
    if (!isTransportBacked) return;
    if (video.paused) {
      await video.play().catch(() => {
        setPlaybackError("Playback could not be started.");
      });
    } else {
      video.pause();
    }
  }, [allowPreviewTransport, isTransportBacked]);

  const seekBy = useCallback((deltaSec: number) => {
    const video = videoRef.current;
    if (isTransportBacked && video) {
      video.currentTime = Math.min(Math.max(video.currentTime + deltaSec, 0), resolvedDuration);
      return;
    }
    if (!allowPreviewTransport) return;
    syncFallbackProgress(progress + deltaSec);
  }, [allowPreviewTransport, isTransportBacked, progress, resolvedDuration, syncFallbackProgress]);

  const setSeek = (next: number) => {
    const video = videoRef.current;
    if (isTransportBacked && video) {
      video.currentTime = next;
      return;
    }
    if (!allowPreviewTransport) return;
    syncFallbackProgress(next);
  };

  const setVolumeLevel = (next: number) => {
    const video = videoRef.current;
    if (isTransportBacked && video) {
      video.volume = next;
      video.muted = next === 0;
      return;
    }
    setVolume(next);
    setMuted(next === 0);
  };

  const toggleMute = useCallback(() => {
    const video = videoRef.current;
    if (isTransportBacked && video) {
      video.muted = !video.muted;
      return;
    }
    setMuted((current) => !current);
  }, [isTransportBacked]);

  const toggleFullscreen = useCallback(async () => {
    const container = containerRef.current;
    if (!container) return;
    if (document.fullscreenElement === container) {
      await document.exitFullscreen().catch(() => {});
      return;
    }
    await container.requestFullscreen?.().catch(() => {
      setFullscreen((current) => !current);
    });
  }, []);

  useEffect(() => {
    const onFullscreenChange = () => {
      setFullscreen(document.fullscreenElement === containerRef.current);
    };

    const onKey = (event: KeyboardEvent) => {
      const video = videoRef.current;
      if (event.key === " " || event.key === "k") {
        event.preventDefault();
        void togglePlay();
      } else if (event.key === "ArrowRight") {
        seekBy(10);
      } else if (event.key === "ArrowLeft") {
        seekBy(-10);
      } else if (event.key === "m") {
        toggleMute();
      } else if (event.key === "f") {
        void toggleFullscreen();
      } else if (!isTransportBacked && event.key === "s" && video) {
        video.currentTime = Math.min((video.currentTime || 0) + 30, mediaDuration);
      }
    };

    document.addEventListener("fullscreenchange", onFullscreenChange);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("fullscreenchange", onFullscreenChange);
      window.removeEventListener("keydown", onKey);
    };
  }, [isTransportBacked, mediaDuration, seekBy, toggleFullscreen, toggleMute, togglePlay]);

  return (
    <div
      ref={containerRef}
      className={`ls-player ${fullscreen ? "ls-player--fullscreen" : ""} ${showControls ? "ls-player--controls" : ""}`}
      onMouseMove={scheduleHide}
      onMouseLeave={() => playing && setShowControls(false)}
    >
      <div className="ls-player__screen" onClick={() => void togglePlay()} role="button" tabIndex={0} aria-label={playing ? "Pause" : "Play"}>
        {isTransportBacked ? (
          <video
            ref={videoRef}
            className="ls-player__video"
            poster={poster}
            preload="metadata"
            playsInline
          />
        ) : (
          <div className="ls-player__fallback-poster" style={{ backgroundImage: `url(${poster})` }} />
        )}
        <div className="ls-player__vignette" />
        {!playing && (isTransportBacked || allowPreviewTransport) && (
          <div className="ls-player__big-play">
            <Play size={48} strokeWidth={1.5} fill="currentColor" />
          </div>
        )}
        {subsOn && playing && subtitle ? (
          <div className="ls-player__subs">{subtitle}</div>
        ) : null}
        {playbackError ? <div className="ls-player__error">{playbackError}</div> : null}
      </div>

      <div className="ls-player__ui">
        <div className="ls-player__top">
          <div>
            <div className="ls-player__kicker mono">NOW PLAYING</div>
            <div className="ls-player__title">{title}</div>
          </div>
          <div className="ls-player__quality mono">
            {isTransportBacked ? "HLS · Signed session" : "Preview transport pending"}
          </div>
        </div>

        <div className="ls-player__bottom">
          <div className="ls-player__timeline">
            <input
              type="range"
              min={0}
              max={resolvedDuration}
              value={Math.min(progress, resolvedDuration)}
              onChange={(event) => setSeek(Number(event.target.value))}
              className="ls-player__range"
              aria-label="Seek"
            />
            <div className="ls-player__track">
              <div className="ls-player__track-fill" style={{ width: `${pct}%` }} />
            </div>
            <div className="ls-player__times mono">
              <span>{formatDuration(progress)}</span>
              <span className="ghost">/</span>
              <span>{formatDuration(resolvedDuration)}</span>
            </div>
          </div>

          <div className="ls-player__controls">
            <div className="ls-player__controls-left">
              <button type="button" onClick={() => seekBy(-10)} aria-label="Back 10 seconds">
                <SkipBack size={18} strokeWidth={1.75} />
              </button>
              <button
                type="button"
                className="ls-player__play-btn"
                onClick={() => void togglePlay()}
                aria-label={playing ? "Pause" : "Play"}
              >
                {playing ? (
                  <Pause size={18} strokeWidth={2} fill="currentColor" />
                ) : (
                  <Play size={18} strokeWidth={2} fill="currentColor" />
                )}
              </button>
              <button type="button" onClick={() => seekBy(10)} aria-label="Forward 10 seconds">
                <SkipForward size={18} strokeWidth={1.75} />
              </button>
              <div className="ls-player__volume">
                <button type="button" onClick={toggleMute} aria-label={muted ? "Unmute" : "Mute"}>
                  {muted || volume === 0 ? (
                    <VolumeX size={18} strokeWidth={1.75} />
                  ) : (
                    <Volume2 size={18} strokeWidth={1.75} />
                  )}
                </button>
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.01}
                  value={muted ? 0 : volume}
                  onChange={(event) => setVolumeLevel(Number(event.target.value))}
                  className="ls-player__vol-range"
                  aria-label="Volume"
                />
              </div>
            </div>
            <div className="ls-player__controls-right">
              <button
                type="button"
                className={subsOn ? "is-active" : ""}
                onClick={() => setSubsOn((current) => !current)}
                aria-label="Subtitles"
              >
                <Subtitles size={18} strokeWidth={1.75} />
              </button>
              <button type="button" onClick={() => void toggleFullscreen()} aria-label={fullscreen ? "Exit fullscreen" : "Fullscreen"}>
                {fullscreen ? (
                  <Minimize2 size={18} strokeWidth={1.75} />
                ) : (
                  <Maximize2 size={18} strokeWidth={1.75} />
                )}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
