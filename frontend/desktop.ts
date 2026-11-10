import { invoke } from "@tauri-apps/api/core";
import { biomeStatus, saveBiome, type BiomeSettings } from "./biome";
import defaults from "./desktop-defaults.json";
import { control, saveTuning, status, type Status, type Tuning } from "./engine";

const key = "acrux.desktop.preferences";
type Bag = Record<string, unknown>;
type Position = [number, number];
type Place = "fish" | "sell";
type Waypoint = { pos: Position | null; time: number | null };
type Location = { name: string; feat: string; key: string; points: Waypoint[] };
type Movement = Bag & { places: Location[] };
interface Config extends Bag {
  macro_on: boolean;
  mfish: Bag;
  base: Bag;
  move: Movement;
  guild_ids: string[];
  channel_ids: string[];
  names: Record<string, string>;
  channels: Record<string, { name: string; guild: string; guildId: string | null }>;
}

function load(): Config {
  let saved = {};
  try { saved = JSON.parse(localStorage.getItem(key) ?? "{}") ?? {}; } catch {}
  const config: Config = { ...structuredClone(defaults), ...saved, macro_on: false };
  for (const [key, name, feat, count] of [
    ["fish_spot", "낚시 장소", "mfish", 1], ["sell_spot", "물고기 판매 장소", "mfish", 1],
    ["craft_spot", "포션 제작 장소", "mcraft", 2], ["match_spot", "메모리 매치 장소", "mmatch", 1],
  ] as const) {
    if (!config.move.places.some(place => place.key === key)) {
      config.move.places.push({ key, name, feat, points: Array.from({ length: count }, () => ({ pos: null, time: null })) });
    }
  }
  return config;
}

const config = load();
const saved = () => ({ config: structuredClone(config) });
const persist = () => localStorage.setItem(key, JSON.stringify({ ...config, macro_on: false }));
const stopped = ["stopped", "중지됨"];
const tuningKeys = {
  bite_max: "biteSeconds",
  lead_ms: "leadMs",
  target_pct: "targetPercent",
  click_gap_ms: "clickGapMs",
  cast_retry: "castRetries",
} as const satisfies Record<string, keyof Tuning>;

function applyStatus(s: Status) {
  const positions = s.positions;
  Object.assign(config.mfish, {
    panel_region: positions?.panel ?? null,
    reel_region: positions?.reel ?? null,
    fish_btn: positions?.fish ?? null,
    bar_region: positions?.bar ?? null,
  });
  for (const [ui, rust] of Object.entries(tuningKeys)) config.mfish[ui] = s.tuning[rust];
}

function reportError(message: string) {
  window.dispatchEvent(new CustomEvent("acrux-desktop-error", { detail: message }));
}

async function syncFishing() {
  const current = await status();
  const shouldRun = config.macro_on && config.mfish.enabled === true;
  if (current.phase === "calibrating") return;
  if (shouldRun && current.phase === "idle") {
    await configureDesktop();
    await control("start");
  }
  if (!shouldRun && current.phase !== "idle") await control("stop");
}

async function configureDesktop() {
  const positions = Object.fromEntries([
    ...["chat_pos", "collection_pos", "collection_close", "dialog_pos"].map(key => [key, config.base[key] ?? null]),
    ...["sell_fish_pos", "first_fish_pos", "sell_all_pos", "confirm_sell_pos", "shop_close_pos"].map(key => [key, config.mfish[key] ?? null]),
  ]);
  await invoke("desktop_navigation", { setup: {
    movement: config.move, positions, sell_max: Math.trunc(Number(config.mfish.sell_max ?? 100)), sell_delay: config.mfish.sell_delay ?? 0,
  } });
}

function placeAt(index: number): Place {
  switch (config.move.places[index]?.key) {
    case "fish_spot": return "fish";
    case "sell_spot": return "sell";
    default: throw new Error("낚시 장소와 판매 장소의 이동을 지원합니다.");
  }
}

let measuring: { place: number; point: number } | undefined;
let pickingWaypoint = false;
async function moveTask(task: string) {
  await configureDesktop();
  await invoke("desktop_task", { task });
  return { ok: true };
}

async function waypoint(payload: Bag, measure: boolean) {
  const i = Number(payload.place);
  const j = Number(payload.point);
  const place = placeAt(i);
  const point = config.move.places[i]?.points[j];
  if (!point) return { error: "지점을 찾을 수 없습니다." };
  const fromBase = payload.from_base !== false;
  if (measure) {
    await configureDesktop();
    await invoke("desktop_waypoint", { place, index: j, measure: true, fromBase });
    measuring = { place: i, point: j };
    return { ok: true };
  }
  pickingWaypoint = true;
  try {
    if (fromBase) {
      await configureDesktop();
      await invoke("desktop_waypoint", { place, index: j, measure: false, fromBase });
      let s: Status;
      do {
        await new Promise(resolve => setTimeout(resolve, 250));
        s = await status();
      } while (s.phase === "moving" && pickingWaypoint);
      if (!pickingWaypoint || s.message !== "이동 동작이 끝났습니다.") return { error: s.message };
    }
    const pos = await invoke<Position>("desktop_pick");
    const current = config.move.places[i]?.points[j];
    if (current !== point) return { error: "지점 목록이 바뀌었습니다. 다시 지정하세요." };
    point.pos = pos;
    point.time = null;
    persist();
    return { move: structuredClone(config.move) };
  } finally {
    pickingWaypoint = false;
  }
}

function biomeSettings(source: Bag): BiomeSettings {
  return {
    enabled: !!source.enabled,
    webhooks: (source.webhooks as string[] ?? []).map(value => value.trim()).filter(Boolean),
    psLink: String(source.ps_link ?? ""),
    player: String(source.player ?? ""),
    everyone: true,
    mentions: source.everyone as Record<string, boolean> ?? {},
    roles: source.roles as Record<string, string> ?? {},
  };
}

async function setConfig(patch: Bag) {
  if (patch.biome) {
    const source = patch.biome as Bag;
    try {
      await saveBiome(biomeSettings(source));
    } catch (error) { reportError(String(error)); return saved(); }
  }
  if ("online_share" in patch) await invoke("save_online", { share: patch.online_share !== false });
  const fish = patch.mfish as Bag | undefined;
  if (fish) {
    const current = await status();
    const tuning = { ...current.tuning };
    for (const [ui, rust] of Object.entries(tuningKeys)) {
      const next = fish[ui];
      if (typeof next === "number") tuning[rust] = next;
    }
    if (JSON.stringify(tuning) !== JSON.stringify(current.tuning)) {
      try { await saveTuning(tuning); } catch (error) {
        reportError(String(error));
        for (const [ui, rust] of Object.entries(tuningKeys)) fish[ui] = current.tuning[rust];
      }
    }
  }
  Object.assign(config, patch);
  if ("macro_on" in patch || fish) {
    try { await syncFishing(); } catch (error) {
      config.macro_on = false;
      reportError(String(error));
    }
  }
  if (config.macro_on && config.mfish.enabled !== true) {
    config.macro_on = false;
    reportError("현재 미리보기에서는 자동 낚시를 켜야 매크로를 시작할 수 있습니다.");
  }
  persist();
  return saved();
}

let last = "";
let seq = 0;
async function desktopApi(name: string, payload: Bag) {
  try { return await request(name, payload); }
  catch (error) { return { error: error instanceof Error ? error.message : String(error) }; }
}

async function request(name: string, payload: Bag) {
  switch (name) {
    case "state": {
      const s = await status();
      let biome = await biomeStatus();
      if (!localStorage.getItem("acrux.desktop.biome-migrated")) {
        if (!biome.settings.enabled && !biome.settings.player && !biome.settings.webhooks.length && config.biome) {
          biome = await saveBiome(biomeSettings(config.biome as Bag));
        }
        localStorage.setItem("acrux.desktop.biome-migrated", "true");
      }
      if (!localStorage.getItem("acrux.desktop.online-migrated")) {
        await invoke("save_online", { share: config.online_share !== false });
        localStorage.setItem("acrux.desktop.online-migrated", "true");
      }
      config.biome = { enabled: biome.settings.enabled, player: biome.settings.player, ps_link: biome.settings.psLink, webhooks: biome.settings.webhooks, everyone: biome.settings.mentions, roles: biome.settings.roles };
      const online = await invoke<{ share: boolean }>("online_status");
      config.online_share = online.share;
      applyStatus(s);
      return { ...saved(), version: s.version, status: stopped, armed: false };
    }
    case "poll": {
      const s = await status();
      const biome = await biomeStatus();
      const online = await invoke<{ online: number | null }>("online_status");
      if (measuring && s.measuredSeconds !== null) {
        const point = config.move.places[measuring.place]?.points[measuring.point];
        if (point) {
          point.time = Math.round(s.measuredSeconds * 100) / 100;
          persist();
        }
        measuring = undefined;
      }
      if (s.phase === "idle") measuring = undefined;
      const move = {
        running: s.phase === "moving" || s.phase === "selling",
        msg: s.message,
        measuring: measuring && s.measuringElapsed !== null ? { ...measuring, since: Date.now() / 1000 - s.measuringElapsed } : null,
      };
      if (s.phase === "idle" && config.macro_on && config.mfish.enabled) config.macro_on = false;
      const logs = s.message === last ? [] : [{ seq: ++seq, time: Date.now() / 1000, msg: s.message, color: "d" }];
      last = s.message;
      return {
        biome: { current: biome.current, history: biome.history, owner: biome.owner, log: biome.watching, muted: false },
        online: online.online, online_on: true,
        seq, logs, events: [], status: stopped, count: 0, armed: false,
        roblox: s.gameAvailable, macro_on: config.macro_on, move,
        mfish: { running: s.phase !== "idle", msg: s.message, stats: s.stats },
        names: config.names, channels: config.channels,
      };
    }
    case "webhook_test":
      await invoke("biome_test");
      return { queued: true };
    case "set_config":
      return setConfig(payload.patch as Bag);
    case "mfish_autocal": {
      const current = await status();
      if (current.phase === "calibrating") {
        await control("stop");
        return { error: "자동 보정 취소" };
      }
      await control("calibrate");
      let s: Status;
      do {
        await new Promise(resolve => setTimeout(resolve, 250));
        s = await status();
      } while (s.phase === "calibrating");
      if (!s.calibrated || s.message !== "자동 보정 완료") return { error: s.message };
      applyStatus(s);
      return { mfish: { ...config.mfish }, done: "낚시 대기 창 · 미니게임 창" };
    }
    case "move_base":
      return moveTask("base");
    case "move_place":
      return moveTask(placeAt(Number(payload.place)) === "fish" ? "fish" : "shop");
    case "sell_test":
      return moveTask("sell");
    case "move_pick":
      return waypoint(payload, false);
    case "move_test":
      return waypoint(payload, true);
    case "move_get":
      return { move: structuredClone(config.move) };
    case "move_arrive":
      await invoke("desktop_arrive");
      return { ok: true };
    case "move_stop":
      measuring = undefined;
      pickingWaypoint = false;
      await invoke("desktop_cancel_pick");
      await control("stop");
      return { ok: true };
    case "pick_cancel":
      if (pickingWaypoint) await control("stop");
      pickingWaypoint = false;
      await invoke("desktop_cancel_pick");
      return {};
    case "mpos_point": {
      const feat = String(payload.feat);
      const key = String(payload.key);
      if (!["base", "mfish", "mmerch", "mcraft", "mmatch"].includes(feat) || !/^[a-z_]+$/.test(key)) {
        return { error: "저장할 위치를 확인하세요." };
      }
      if (feat === "mfish" && ["fish_btn", "close_pos", "title_pos"].includes(key)) {
        return { error: "낚시 위치는 자동 보정을 사용하세요." };
      }
      const pos = await invoke<Position>("desktop_pick");
      const group = config[feat] as Bag;
      group[key] = pos;
      persist();
      return { [feat]: structuredClone(group), pos };
    }
    case "mfish_stop":
      if ((await status()).phase !== "calibrating") await control("stop");
      config.macro_on = false;
      return {};
    case "mfish_check":
      return (await status()).calibrated ? { bar: "보정됨" } : { error: "자동 보정을 먼저 실행하세요." };
    case "add_target":
    case "remove_target": {
      const id = String(payload.id ?? "").trim();
      const guild = payload.kind === "guild";
      if (!/^\d+$/.test(id) || (!guild && payload.kind !== "channel")) return { error: "숫자 ID만 입력 가능" };
      const ids = guild ? config.guild_ids : config.channel_ids;
      const position = ids.indexOf(id);
      if (name === "add_target" && position === -1) {
        ids.push(id);
        const label = String(payload.name ?? "").trim();
        if (label && guild) config.names[id] = label;
        if (label && !guild) config.channels[id] = { name: label, guild: String(payload.guild ?? ""), guildId: payload.guildId ? String(payload.guildId) : null };
      }
      if (name === "remove_target" && position >= 0) {
        ids.splice(position, 1);
        if (guild) delete config.names[id];
        else delete config.channels[id];
      }
      persist();
      return saved();
    }
    case "prime_recent":
    case "mpop_stop":
      return {};
    default: {
      const error = "이 기능의 실행은 아직 Tauri 미리보기에 연결되지 않았습니다.";
      reportError(error);
      return { error };
    }
  }
}

const desktopWindow = window as Window & { acruxDesktopApi?: typeof desktopApi };
desktopWindow.acruxDesktopApi = desktopApi;
const script = document.createElement("script");
script.src = new URL("app.js", location.href).href;
document.body.append(script);
