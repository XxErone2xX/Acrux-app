import { invoke } from "@tauri-apps/api/core";

export type Phase = "idle" | "calibrating" | "searching" | "casting" | "waiting" | "reeling" | "result" | "moving" | "selling" | "recording";
export type Action = "calibrate" | "start" | "stop" | "testFish";
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

export type FeatureName = "mfish" | "mpop" | "mblood" | "mitem" | "mmerch" | "mcraft" | "mmatch";
export interface FeatureStatus {
  running: boolean;
  msg: string;
  error?: string | null;
  completedAt?: number;
  [key: string]: unknown;
}

export interface Status {
  platform: string;
  generation: number;
  version: string;
  captureReady: boolean;
  inputReady: boolean;
  gameAvailable: boolean;
  macroRunning: boolean;
  returning: boolean;
  featureStatuses: Record<FeatureName, FeatureStatus>;
  connectionMessage?: string;
  calibrated: boolean;
  positions: { panel: number[] | null; reel: number[] | null; fish: number[]; bar: number[]; result?: number[] | null; close?: number[] | null; title?: number[] | null } | null;
  phase: Phase;
  message: string;
  settingsError: string | null;
  stats: Stats;
  totals: Stats;
  tuning: Tuning;
  recording?: { place: number; elapsed: number } | null;
  measuringElapsed: number | null;
  measuredSeconds: number | null;
}

export const status = () => invoke<Status>("status");
export const control = (action: Action, generation?: number) => invoke<Status>("control", { action, generation });
export const macroAction = (action: string, payload: Record<string, unknown> = {}, generation?: number) => invoke<Status>("macro_action", { action, payload, generation });
export const saveTuning = (tuning: Tuning) => invoke<Status>("save_tuning", { tuning });
