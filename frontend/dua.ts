import { invoke } from "@tauri-apps/api/core";
import type { Status } from "./engine";

export type Destination = "base" | "coast" | "sell" | "craft" | "match";
export type Rect = [number, number, number, number];
export interface Observation {
  image: { pixels: Uint8Array; width: number; height: number; origin: [number, number] };
  words: [string, number, number, number, number][];
  capturedAt: number;
  inputDeadline: number;
  generation: number;
  engine: string;
  viewport: [number, number];
}

export async function duaCall<T = unknown>(method: string, args: unknown[] = [], kwargs = {}): Promise<T> {
  const { generation } = await invoke<Status>("status");
  return invoke<T>("automation_action", {
    action: "sdk_call", payload: { module: "dua", method, args, kwargs }, generation,
  });
}

export const dua = Object.freeze({
  call: duaCall,
  capabilities: () => duaCall<Record<string, unknown>>("capabilities"),
  status: () => duaCall<Status>("status"),
  logs: () => duaCall<string[]>("logs"),
  start: (features: string[]) => duaCall<Status>("start", [features]),
  stop: () => duaCall<Status>("stop"),
  go: (destination: Destination) => duaCall("go", [destination]),
  mark: (destination: Destination) => duaCall("mark", [destination]),
  cancel: () => duaCall("cancel"),
  tap: (x: number, y: number, ms = 40) => duaCall("tap", [x, y, ms]),
  key: (key: string, ms = 40) => duaCall("key_tap", [key, ms]),
  text: (text: string, submit = false) => duaCall("text", [text], { submit }),
  gesture: (gesture: Record<string, unknown>) => duaCall("gesture", [gesture]),
  open: (url: string) => duaCall("open_link", [url]),
  async observe(rect: Rect | null = null): Promise<Observation> {
    const raw = await duaCall<Omit<Observation, "image"> & {
      image: Omit<Observation["image"], "pixels"> & { __bgra__: string };
    }>("observe", [rect]);
    const { __bgra__, ...image } = raw.image;
    const bytes = atob(__bgra__);
    const pixels = Uint8Array.from(bytes, char => char.charCodeAt(0));
    if (pixels.length !== image.width * image.height * 4) throw new Error("게임 화면 크기나 픽셀 데이터가 올바르지 않습니다.");
    return { ...raw, image: { ...image, pixels } };
  },
});

declare global { interface Window { AcruxDUA: typeof dua } }
window.AcruxDUA = dua;
