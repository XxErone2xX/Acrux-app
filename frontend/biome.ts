import { invoke } from "@tauri-apps/api/core";

export interface BiomeSettings {
  enabled: boolean;
  webhooks: string[];
  psLink: string;
  everyone: boolean;
  mentions: Record<string, boolean>;
  roles: Record<string, string>;
  player: string;
  safeMode: boolean;
  safeV2: boolean;
}
export interface BiomeStatus {
  settings: BiomeSettings;
  current: string | null;
  watching: boolean;
  message: string;
  logs: string[];
  history: [number, string][];
  owner: string;
  job: string | null;
  safe: boolean;
  safeLeft: number;
  held: string | null;
}
export const defaultBiome: BiomeSettings = {
  enabled: false, webhooks: [], psLink: "", everyone: true, mentions: {}, roles: {}, player: "", safeMode: true, safeV2: true,
};
export const biomeStatus = () => invoke<BiomeStatus>("biome_status");
export const saveBiome = (settings: BiomeSettings) => invoke<BiomeStatus>("save_biome", { settings });
