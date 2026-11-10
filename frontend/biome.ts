import { invoke } from "@tauri-apps/api/core";

export interface BiomeSettings {
  enabled: boolean;
  webhooks: string[];
  psLink: string;
  everyone: boolean;
  mentions: Record<string, boolean>;
  roles: Record<string, string>;
  player: string;
}
export interface BiomeStatus {
  settings: BiomeSettings;
  current: string | null;
  watching: boolean;
  message: string;
  logs: string[];
  history: [number, string][];
  owner: string;
}
export const defaultBiome: BiomeSettings = {
  enabled: false, webhooks: [], psLink: "", everyone: true, mentions: {}, roles: {}, player: "",
};
export const biomeStatus = () => invoke<BiomeStatus>("biome_status");
export const saveBiome = (settings: BiomeSettings) => invoke<BiomeStatus>("save_biome", { settings });
