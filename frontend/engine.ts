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
}

export const status = () => invoke<Status>("status");
export const control = (action: Action) => invoke<Status>("control", { action });
export const saveTuning = (tuning: Tuning) => invoke<Status>("save_tuning", { tuning });
