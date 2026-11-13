// VideoPlayer: the Gallery viewer's player for a video or audio file, with the app's own controls (ADR-0023): play,
// seek, ±10 s, previous and next frame, volume and mute (kept between runs), speed, loop and full screen, each with a
// key. The controls hide while it plays and the pointer rests. A file the browser can't play says so.

import { Button, Icon, IconButton, Menu, Slider } from "iro-ui";
import {
  AudioBarsIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ExpandIcon,
  PauseIcon,
  PlayIcon,
  RefreshIcon,
  ShrinkIcon,
  SkipBackIcon,
  SkipForwardIcon,
  VolumeIcon,
  VolumeOffIcon,
} from "iro-ui/icons";
import { useCallback, useEffect, useRef, useState } from "react";
import { useSettings } from "../../app/SettingsProvider";
import { formatDuration } from "../../lib/format";

// ── Constants ─────────────────────────────
export const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
/** A frame's length until the player has measured one: 30 fps. */
const DEFAULT_FRAME = 1 / 30;
const JUMP = 10;
const VOLUME_STEP = 0.05;
/** How long the pointer rests before the controls hide while playing. */
const IDLE_MS = 2500;

// ── Types ─────────────────────────────────
interface Props {
  src: string;
  name: string;
  audio: boolean;
  speed: number;
  onSpeedChange: (speed: number) => void;
  /** Opens the file in its app: offered when it can't play here. */
  onOpenInApp: () => void;
}

// ── Private helpers ───────────────────────
/** Keys typed into a control (a slider's arrows, a button's space) are that control's. */
function ownedByControl(event: KeyboardEvent): boolean {
  const target = event.target as HTMLElement | null;
  if (!target) return false;
  if (target.closest("[role=slider], input, select, textarea")) return true;
  return (event.key === " " || event.key === "Enter") && target.closest("button, [role=menuitem]") != null;
}

// ── Public API ────────────────────────────
/** The player. */
export function VideoPlayer({ src, name, audio, speed, onSpeedChange, onOpenInApp }: Props) {
  const { settings, savePlayer } = useSettings();
  const video = useRef<HTMLVideoElement>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const frame = useRef(DEFAULT_FRAME);
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(settings?.player.volume ?? 1);
  const [muted, setMuted] = useState(settings?.player.muted ?? false);
  const [loop, setLoop] = useState(false);
  const [full, setFull] = useState(false);
  const [idle, setIdle] = useState(false);
  const [failed, setFailed] = useState(false);
  const idleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // The element follows the state; the state is what's saved and shown.
  useEffect(() => {
    if (!video.current) return;
    video.current.volume = volume;
    video.current.muted = muted;
  }, [volume, muted]);
  useEffect(() => {
    if (video.current) video.current.playbackRate = speed;
  }, [speed]);

  // A frame's length, from the frames the browser shows: the shortest gap between two.
  useEffect(() => {
    const element = video.current;
    if (!element || audio || !("requestVideoFrameCallback" in element)) return;
    let last: number | null = null;
    let handle = 0;
    const tick: VideoFrameRequestCallback = (_, meta) => {
      if (last != null) {
        const gap = meta.mediaTime - last;
        if (gap > 0.001 && gap < 0.2)
          frame.current = Math.min(frame.current === DEFAULT_FRAME ? gap : frame.current, gap);
      }
      last = meta.mediaTime;
      handle = element.requestVideoFrameCallback(tick);
    };
    handle = element.requestVideoFrameCallback(tick);
    return () => element.cancelVideoFrameCallback(handle);
  }, [audio]);

  useEffect(() => {
    const follow = () => setFull(document.fullscreenElement === wrap.current);
    document.addEventListener("fullscreenchange", follow);
    return () => document.removeEventListener("fullscreenchange", follow);
  }, []);

  const toggle = useCallback(() => {
    const element = video.current;
    if (!element) return;
    if (element.paused) void element.play().catch(() => {});
    else element.pause();
  }, []);

  const seekTo = useCallback((at: number) => {
    const element = video.current;
    if (!element) return;
    element.currentTime = Math.min(Math.max(at, 0), element.duration || 0);
    setTime(element.currentTime);
  }, []);

  const stepFrame = useCallback(
    (by: number) => {
      video.current?.pause();
      seekTo((video.current?.currentTime ?? 0) + by * frame.current);
    },
    [seekTo],
  );

  const changeVolume = useCallback(
    (next: number, save: boolean) => {
      const level = Math.min(Math.max(next, 0), 1);
      setVolume(level);
      setMuted(level === 0);
      if (save) savePlayer({ volume: level, muted: level === 0 });
    },
    [savePlayer],
  );

  const toggleMute = useCallback(() => {
    setMuted(!muted);
    savePlayer({ volume, muted: !muted });
  }, [savePlayer, volume, muted]);

  const toggleFull = useCallback(() => {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void wrap.current?.requestFullscreen().catch(() => {});
  }, []);

  const wake = useCallback(() => {
    setIdle(false);
    if (idleTimer.current) clearTimeout(idleTimer.current);
    idleTimer.current = setTimeout(() => setIdle(true), IDLE_MS);
  }, []);
  useEffect(() => () => void (idleTimer.current && clearTimeout(idleTimer.current)), []);

  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      if (ownedByControl(event) || event.ctrlKey || event.metaKey || event.altKey) return;
      const at = video.current?.currentTime ?? 0;
      const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
      switch (key) {
        case " ":
        case "k":
          toggle();
          break;
        case "j":
          seekTo(at - JUMP);
          break;
        case "l":
          seekTo(at + JUMP);
          break;
        case ",":
          stepFrame(-1);
          break;
        case ".":
          stepFrame(1);
          break;
        case "ArrowUp":
          changeVolume(volume + VOLUME_STEP, true);
          break;
        case "ArrowDown":
          changeVolume(volume - VOLUME_STEP, true);
          break;
        case "m":
          toggleMute();
          break;
        case "f":
          toggleFull();
          break;
        case "Home":
          seekTo(0);
          break;
        case "End":
          seekTo(Number.POSITIVE_INFINITY);
          break;
        default:
          return;
      }
      event.preventDefault();
      wake();
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  }, [toggle, seekTo, stepFrame, changeVolume, toggleMute, toggleFull, wake, volume]);

  if (failed) {
    return (
      <div className="player-failed">
        <span>This file can't play here.</span>
        <Button onClick={onOpenInApp}>Open in app</Button>
      </div>
    );
  }

  return (
    <div
      ref={wrap}
      className="player"
      data-audio={audio || undefined}
      data-idle={(idle && playing && !audio) || undefined}
      onPointerMove={wake}
      onPointerDown={wake}
    >
      {/* biome-ignore lint/a11y/useMediaCaption: library files come without captions */}
      <video
        ref={video}
        className="player-video"
        src={src}
        autoPlay
        loop={loop}
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onTimeUpdate={(event) => setTime(event.currentTarget.currentTime)}
        onLoadedMetadata={(event) => {
          setDuration(event.currentTarget.duration);
          event.currentTarget.playbackRate = speed;
        }}
        onError={() => setFailed(true)}
        onClick={toggle}
        onDoubleClick={toggleFull}
      />
      {audio && (
        <div className="player-audio" aria-hidden>
          <Icon icon={AudioBarsIcon} size={48} />
          <span>{name}</span>
        </div>
      )}

      <div className="player-controls">
        <Slider
          className="player-seek"
          aria-label="Position"
          min={0}
          max={duration || 0}
          step={0.01}
          value={time}
          onValueChange={seekTo}
        />
        <div className="player-row">
          <IconButton
            icon={<Icon icon={playing ? PauseIcon : PlayIcon} />}
            label={playing ? "Pause" : "Play"}
            shortcut="Space"
            onClick={toggle}
          />
          <IconButton
            icon={<Icon icon={SkipBackIcon} />}
            label="Back 10 seconds"
            shortcut="J"
            onClick={() => seekTo(time - JUMP)}
          />
          {!audio && (
            <>
              <IconButton
                icon={<Icon icon={ChevronLeftIcon} />}
                label="Previous frame"
                shortcut=","
                onClick={() => stepFrame(-1)}
              />
              <IconButton
                icon={<Icon icon={ChevronRightIcon} />}
                label="Next frame"
                shortcut="."
                onClick={() => stepFrame(1)}
              />
            </>
          )}
          <IconButton
            icon={<Icon icon={SkipForwardIcon} />}
            label="Forward 10 seconds"
            shortcut="L"
            onClick={() => seekTo(time + JUMP)}
          />
          <span className="player-time">
            {formatDuration(time)} / {formatDuration(duration)}
          </span>
          <span className="player-spacer" />
          <IconButton
            icon={<Icon icon={muted || volume === 0 ? VolumeOffIcon : VolumeIcon} />}
            label={muted ? "Unmute" : "Mute"}
            shortcut="M"
            onClick={toggleMute}
          />
          <Slider
            className="player-volume"
            aria-label="Volume"
            min={0}
            max={100}
            value={muted ? 0 : Math.round(volume * 100)}
            onValueChange={(value) => changeVolume(value / 100, false)}
            onValueCommit={(value) => changeVolume(value / 100, true)}
          />
          <Menu
            trigger={
              <Button size="sm" variant="ghost" aria-label="Speed">
                {speed}×
              </Button>
            }
            side="top"
            items={SPEEDS.map((each) => ({
              label: each === 1 ? "Normal" : `${each}×`,
              onSelect: () => onSpeedChange(each),
            }))}
          />
          <IconButton icon={<Icon icon={RefreshIcon} />} label="Loop" active={loop} onClick={() => setLoop(!loop)} />
          {!audio && (
            <IconButton
              icon={<Icon icon={full ? ShrinkIcon : ExpandIcon} />}
              label={full ? "Leave full screen" : "Full screen"}
              shortcut="F"
              onClick={toggleFull}
            />
          )}
        </div>
      </div>
    </div>
  );
}
