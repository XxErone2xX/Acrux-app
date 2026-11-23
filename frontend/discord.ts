import { invoke } from "@tauri-apps/api/core";

export interface DiscordState {
  linked: boolean;
  name: string;
  code: string | null;
  error: string;
}

export const discordState = () => invoke<DiscordState>("discord_state");
export const cancelDiscord = () => invoke<{ ok: boolean }>("discord_cancel");
export const unlinkDiscord = () => invoke<{ ok: boolean }>("discord_logout");

export async function linkDiscord() {
  const result = await invoke<{ code: string; ok: boolean }>("discord_login");
  try {
    await invoke("discord_open_page");
  } catch (error) {
    await cancelDiscord();
    throw error;
  }
  return result;
}
