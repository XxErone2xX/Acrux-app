import { invoke } from "@tauri-apps/api/core";

export type Phase = "idle" | "calibrating" | "searching" | "casting" | "waiting" | "reeling" | "result" | "moving" | "selling" | "recording";
export type Action = "calibrate" | "start" | "stop";
export interface Tuning {
  biteSeconds: number;
  leadMs: number;
  targetPercent: number;
  deadband: number;
  clickMs: number;
  clickGapMs: number;
  castRetries: number;
  adaptive: boolean;
  tapAbove: boolean;
}

export interface Stats {
  success: number;
  junk: number;
  fail: number;
  full: number;
}

export interface DuaStatus {
  supported: boolean;
  background: boolean;
  hotkeys: boolean;
  running: boolean;
  route: "headless" | "screenshot" | null;
  reason: string;
  headlessTicks: number;
  screenshotFrames: number;
  transitions: number;
}

export interface SessionStatus {
  supported: boolean;
  state: "stopped" | "starting" | "recovering" | "ready" | "error" | "unsupported";
  message: string;
  width: number;
  height: number;
}

export interface Status {
  platform: string;
  version: string;
  captureReady: boolean;
  inputReady: boolean;
  gameAvailable: boolean;
  calibrated: boolean;
  positions: { panel: number[]; reel: number[]; fish: number[]; bar: number[] } | null;
  phase: Phase;
  message: string;
  settingsError: string | null;
  stats: Stats;
  totals: Stats;
  tuning: Tuning;
  measuringElapsed: number | null;
  measuredSeconds: number | null;
  dua: DuaStatus;
  session: SessionStatus;
  generation: number;
  rememberedDestinations?: string[];
}

export const status = () => invoke<Status>("status");
export const control = (action: Action) => invoke<Status>("control", { action });
export const duaStart = (generation: number) => invoke<Status>("dua_start", { features: ["mfish"], generation });
export const duaStop = () => invoke<Status>("dua_stop");
export const saveTuning = (tuning: Tuning) => invoke<Status>("save_tuning", { tuning });
