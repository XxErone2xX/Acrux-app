import { invoke } from "@tauri-apps/api/core";
import { biomeStatus } from "./biome";
import defaults from "./desktop-defaults.json";
import { control, macroAction, status, type FeatureName, type Status, type Tuning } from "./engine";

const key = "acrux.main-port.preferences";
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
}

function normalize(source: unknown): Config {
  const data = source && typeof source === "object" && !Array.isArray(source) ? source as Bag : {};
  const config = { ...structuredClone(defaults), ...structuredClone(data), macro_on: false } as Config;
  for (const [key, fallback] of Object.entries(defaults)) {
    if (fallback && typeof fallback === "object" && !Array.isArray(fallback)) {
      const value = data[key];
      config[key] = { ...structuredClone(fallback), ...(value && typeof value === "object" && !Array.isArray(value) ? structuredClone(value) : {}) };
    }
  }
  const biome = config.biome as Bag;
  if (!biome.safe_v2) Object.assign(biome, { safe_mode: true, safe_v2: true });
  biome.safe_mode = !!biome.safe_mode;
  if (!Array.isArray(config.move.places)) config.move.places = [];
  for (const [key, name, feat, count] of [
    ["fish_spot", "낚시 장소", "mfish", 1], ["sell_spot", "물고기 판매 장소", "mfish", 1],
    ["craft_spot", "포션 제작 장소", "mcraft", 2], ["match_spot", "메모리 매치 장소", "mmatch", 1],
  ] as const) {
    if (!config.move.places.some(place => place?.key === key)) config.move.places.push({ key, name, feat, points: Array.from({ length: count }, () => ({ pos: null, time: null })) });
  }
  return config;
}

function load(): Config {
  try { return normalize(JSON.parse(localStorage.getItem(key) ?? "{}")); }
  catch { return normalize({}); }
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
  config.macro_on = s.macroRunning === true;
  if (!pendingConfigSaves) desiredMacro = config.macro_on;
  const positions = s.positions;
  if (positions) Object.assign(config.mfish, {
    panel_region: positions.panel,
    reel_region: positions.reel,
    fish_btn: positions.fish,
    bar_region: positions.bar,
  });
  if (positions) for (const [ui, native] of [["result_region", "result"], ["close_pos", "close"], ["title_pos", "title"]] as const) {
    if (positions[native] != null) config.mfish[ui] = positions[native];
  }
  for (const [ui, rust] of Object.entries(tuningKeys)) config.mfish[ui] = s.tuning[rust];
}

function reportError(message: string) {
  window.dispatchEvent(new CustomEvent("acrux-desktop-error", { detail: message }));
}

let loaded: Promise<void> | undefined;
async function loadNativeConfig() {
  if (!loaded) loaded = (async () => {
    const native = await invoke<Bag>("macro_settings");
    if (!native || typeof native !== "object" || Array.isArray(native)) throw new Error("저장된 설정을 읽지 못했습니다.");
    Object.assign(config, normalize(native));
    persist();
  })().catch(error => { loaded = undefined; throw error; });
  return loaded;
}

let configSaves = Promise.resolve();
let macroRevision = 0;
let desiredMacro = false;
let pendingConfigSaves = 0;
async function syncMacro(revision: number) {
  const current = await status();
  if (revision !== macroRevision || !desiredMacro) return;
  if (current.phase === "calibrating") throw new Error("자동 보정을 마친 뒤 시작하세요.");
  if (!current.macroRunning) {
    await configureDesktop();
    if (revision !== macroRevision || !desiredMacro) return;
    const ready = await status();
    if (revision !== macroRevision || !desiredMacro) return;
    const started = await macroAction("start", {}, ready.generation);
    if (revision !== macroRevision || !desiredMacro) applyStatus(await macroAction("stop"));
    else applyStatus(started);
  } else {
    applyStatus(current);
  }
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
    const move = structuredClone(config.move);
    move.places[i].points[j] = { ...point, pos, time: null };
    const applied = await setConfig({ move });
    if ("error" in applied) return applied;
    return { move: structuredClone(config.move) };
  } finally {
    pickingWaypoint = false;
  }
}

async function setConfig(patch: Bag) {
  if (!patch || typeof patch !== "object" || Array.isArray(patch)) return { ...saved(), error: "저장할 설정을 확인하세요." };
  if ("macro_on" in patch) desiredMacro = patch.macro_on === true;
  const revision = ++macroRevision;
  if (patch.macro_on === false) {
    config.macro_on = false;
    pickingWaypoint = false;
    await invoke("desktop_cancel_pick");
    applyStatus(await macroAction("stop"));
    persist();
  }
  pendingConfigSaves++;
  const save = configSaves.then(async () => {
    await loadNativeConfig();
    const next = normalize({ ...structuredClone(config), ...structuredClone(patch) });
    const current = await status();
    if (current.phase !== "idle" || current.macroRunning) await macroAction("stop");
    // Native validation, profile persistence and engine tuning form one transaction.
    const native = await invoke<Config>("save_macro_settings", { config: next });
    Object.assign(config, normalize(native));
    if (revision === macroRevision && desiredMacro) {
      await syncMacro(revision);
    }
    applyStatus(await status());
    persist();
    return saved();
  });
  configSaves = save.then(() => undefined, () => undefined);
  try { return await save; }
  catch (error) {
    if (revision === macroRevision) { desiredMacro = false; config.macro_on = false; }
    persist();
    const message = error instanceof Error ? error.message : String(error);
    reportError(message);
    return { ...saved(), error: message };
  } finally { pendingConfigSaves--; }
}

const calibrations = new Map<string, FeatureName>();
async function runFeatureAction(name: string, payload: Bag) {
  if (name.endsWith("_stop")) {
    applyStatus(await macroAction(name, payload));
    return { ok: true };
  }
  await configSaves;
  await loadNativeConfig();
  await configureDesktop();
  const current = await status();
  const next = await macroAction(name, payload, current.generation);
  applyStatus(next);
  const feature = next.featureStatuses?.[name.split("_")[0] as FeatureName];
  return { ok: true, ...(typeof feature?.name === "string" ? { name: feature.name } : {}) };
}

async function autoCalibrate(name: string, feature: FeatureName) {
  if (calibrations.has(name)) {
    applyStatus(await macroAction(`${feature}_stop`));
    return { error: "자동 보정 취소 중" };
  }
  calibrations.set(name, feature);
  try {
    await configSaves;
    await loadNativeConfig();
    const before = await status();
    let s = await macroAction(name, {}, before.generation);
    applyStatus(s);
    const deadline = Date.now() + 180_000;
    while (s.featureStatuses?.[feature]?.running) {
      if (Date.now() >= deadline) {
        applyStatus(await macroAction(`${feature}_stop`));
        return { error: "자동 보정 대기 시간이 지났습니다." };
      }
      await new Promise(resolve => setTimeout(resolve, 250));
      s = await status();
      applyStatus(s);
    }
    const completed = s.featureStatuses?.[feature];
    if (!completed) return { error: "자동 보정 상태를 읽지 못했습니다." };
    if (completed.error) return { error: completed.error };
    if (!(Number(completed.completedAt) > Number(before.featureStatuses?.[feature]?.completedAt ?? 0))) return { error: completed.msg || "자동 보정이 완료되지 않았습니다." };
    const native = normalize(await invoke<Config>("macro_settings"));
    // Keep unrelated edits made while calibration was running.
    config[feature] = native[feature];
    if (name === "sell_autocal") config.base = native.base;
    persist();
    return { [feature]: structuredClone(config[feature]), ...(name === "sell_autocal" ? { base: structuredClone(config.base) } : {}) };
  } finally {
    calibrations.delete(name);
  }
}

function snapshotPixels(buffer: ArrayBuffer) {
  if (!(buffer instanceof ArrayBuffer) || buffer.byteLength < 8) throw new Error("게임 화면을 읽지 못했습니다.");
  const view = new DataView(buffer), width = view.getUint32(0, true), height = view.getUint32(4, true);
  const size = width * height * 4;
  if (width < 1 || height < 1 || width > 16384 || height > 16384 || size > 256 * 1024 * 1024 || buffer.byteLength !== size + 8) throw new Error("게임 화면 크기가 올바르지 않습니다.");
  return { width, height, pixels: new Uint8ClampedArray(buffer, 8) };
}

let viewOpen = false;
async function showGameFrame() {
  if (viewOpen) return;
  viewOpen = true;
  try {
    const image = snapshotPixels(await invoke<ArrayBuffer>("desktop_snapshot"));
    const dialog = document.createElement("dialog");
    dialog.className = "card";
    dialog.setAttribute("aria-label", "게임 화면 확인");
    Object.assign(dialog.style, { maxWidth: "94vw", maxHeight: "94vh", background: "var(--panel)", color: "var(--fg)", border: "1px solid var(--line)" });
    const title = document.createElement("h3"); title.textContent = "게임 화면 확인";
    const hint = document.createElement("p"); hint.className = "hint"; hint.textContent = "캡처한 화면입니다. 게임은 별도 Roblox 창에서 직접 조작합니다.";
    const canvas = document.createElement("canvas"); canvas.width = image.width; canvas.height = image.height;
    canvas.setAttribute("aria-label", "캡처한 Roblox 화면");
    Object.assign(canvas.style, { display: "block", maxWidth: "88vw", maxHeight: "70vh", width: "auto", height: "auto" });
    const context = canvas.getContext("2d");
    if (!context) throw new Error("게임 화면을 표시하지 못했습니다.");
    context.putImageData(new ImageData(image.pixels, image.width, image.height), 0, 0);
    const close = document.createElement("button"); close.type = "button"; close.className = "btn ghost"; close.textContent = "닫기";
    let removed = false;
    const remove = () => { if (removed) return; removed = true; dialog.close(); dialog.remove(); viewOpen = false; };
    close.addEventListener("click", remove); dialog.addEventListener("close", remove);
    dialog.addEventListener("cancel", event => { event.preventDefault(); remove(); });
    dialog.append(title, hint, canvas, close); document.body.append(dialog); dialog.showModal(); close.focus();
  } catch (error) { viewOpen = false; throw error; }
}

let connectionState: HTMLElement | undefined;
function updateConnection(s: Status) {
  if (connectionState) connectionState.textContent = s.connectionMessage || (s.gameAvailable ? "게임 창이 연결됐습니다." : "실행 중인 Roblox 창을 기다립니다.");
}

function addConnectionRow() {
  const section = document.querySelector('.sec[data-sec="mf-base"]');
  if (!section) return;
  const card = document.createElement("div"); card.className = "card";
  const heading = document.createElement("b"); heading.textContent = "게임 연결";
  const hint = document.createElement("p"); hint.className = "hint"; hint.textContent = "Roblox는 별도 창에서 직접 조작합니다. 화면 확인으로 캡처를 확인할 수 있습니다.";
  connectionState = document.createElement("p"); connectionState.className = "hint"; connectionState.setAttribute("role", "status"); connectionState.textContent = "실행 중인 Roblox 창을 기다립니다.";
  const buttons = document.createElement("div"); buttons.className = "confirm-btns";
  for (const [title, action] of [["게임 열기", "open_game"], ["화면 확인", "game_view"]]) {
    const button = document.createElement("button"); button.type = "button"; button.className = "btn ghost"; button.textContent = title;
    button.addEventListener("click", async () => {
      button.disabled = true;
      try { const result = await desktopApi(action, {}); if ("error" in result) reportError(String(result.error)); }
      finally { button.disabled = false; }
    });
    buttons.append(button);
  }
  card.append(heading, hint, connectionState, buttons);
  section.insertBefore(card, section.querySelector(".mpos-cards"));
}

const lastMessages = new Map<string, string>();
let seq = 0;
const openedAt = Date.now() / 1000;
function statusLabel(s: Status) {
  if (s.settingsError) return ["error", s.settingsError];
  return s.macroRunning || s.phase !== "idle" ? ["flux", s.message || "실행 중"] : stopped;
}
async function desktopApi(name: string, payload: Bag) {
  try { return await request(name, payload); }
  catch (error) { return { error: error instanceof Error ? error.message : String(error) }; }
}

async function request(name: string, payload: Bag) {
  switch (name) {
    case "state": {
      const failures: string[] = [];
      try { await loadNativeConfig(); } catch (error) { failures.push(String(error)); }
      const [current, biome, online] = await Promise.allSettled([status(), biomeStatus(), invoke<{ share: boolean }>("online_status")]);
      if (current.status === "fulfilled") { applyStatus(current.value); updateConnection(current.value); }
      else failures.push(String(current.reason));
      if (biome.status === "rejected") failures.push(String(biome.reason));
      if (online.status === "fulfilled") config.online_share = online.value.share;
      else failures.push(String(online.reason));
      if (failures.length) reportError(failures.join(" · "));
      return { ...saved(), version: current.status === "fulfilled" ? current.value.version : "개발 버전", status: current.status === "fulfilled" ? statusLabel(current.value) : stopped, armed: false, mods: [] };
    }
    case "poll": {
      const s = await status();
      const [biome, online] = await Promise.allSettled([biomeStatus(), invoke<{ online: number | null }>("online_status")]);
      updateConnection(s);
      applyStatus(s);
      if (measuring && s.measuredSeconds !== null) {
        const move = structuredClone(config.move), point = move.places[measuring.place]?.points[measuring.point];
        if (point) { point.time = Math.round(s.measuredSeconds * 100) / 100; await setConfig({ move }); }
        measuring = undefined;
      }
      if (s.phase === "idle") measuring = undefined;
      const move = {
        running: s.phase === "moving" || s.phase === "selling",
        msg: s.message,
        measuring: measuring && s.measuringElapsed !== null ? { ...measuring, since: Date.now() / 1000 - s.measuringElapsed } : null,
        recording: s.recording ? { place: s.recording.place, since: Date.now() / 1000 - s.recording.elapsed } : null,
      };
      const logs: { seq: number; time: number; msg: string; color: string }[] = [];
      const messages = { engine: { msg: s.message, error: s.settingsError }, ...s.featureStatuses };
      for (const [channel, value] of Object.entries(messages)) {
        const message = value.error || value.msg;
        if (message && lastMessages.get(channel) !== message) logs.push({ seq: ++seq, time: Date.now() / 1000, msg: message, color: value.error ? "r" : "d" });
        lastMessages.set(channel, message || "");
      }
      const stats = (value: Status["stats"]) => ({ fish_success: value.success, fish_junk: value.junk, fish_fail: value.fail, fish_full: value.full });
      const bio = biome.status === "fulfilled" ? biome.value : undefined;
      return {
        biome: bio ? {
          current: bio.current, history: bio.history, owner: bio.owner, log: bio.watching,
          job: bio.job, safe: bio.safe, safe_left: bio.safeLeft, held: bio.held,
        } : null,
        online: online.status === "fulfilled" ? online.value.online : null, online_on: online.status === "fulfilled",
        seq, logs, status: statusLabel(s),
        roblox: s.gameAvailable, macro_on: s.macroRunning, move,
        ...s.featureStatuses,
        mfish: { ...s.featureStatuses?.mfish, stats: s.stats },
        stats: { session: stats(s.stats), all: stats(s.totals), since: null, started: openedAt },
      };
    }
    case "open_game": {
      const current = await invoke<Status>("desktop_open_game"); updateConnection(current); return { ok: true };
    }
    case "game_view":
      await showGameFrame(); return { ok: true };
    case "webhook_test":
      await invoke("biome_test");
      return { queued: true };
    case "set_config":
      return setConfig(payload.patch as Bag);
    case "mpop_test": case "mitem_test": case "mblood_test":
    case "mmerch_check": case "mmerch_buy_test": case "mcraft_test": case "mmatch_test":
    case "mpop_stop": case "mitem_stop": case "mblood_stop": case "mfish_stop":
    case "mmerch_stop": case "mcraft_stop": case "mmatch_stop":
      return runFeatureAction(name, payload);
    case "sell_autocal":
      return autoCalibrate(name, "mfish");
    case "mmerch_autocal":
      return autoCalibrate(name, "mmerch");
    case "mcraft_autocal":
      return autoCalibrate(name, "mcraft");
    case "mmatch_autocal": case "mmatch_boardcal":
      return autoCalibrate(name, "mmatch");
    case "mfish_autocal": {
      const current = await status();
      if (current.phase === "calibrating") {
        await control("stop");
        return { error: "자동 보정 취소" };
      }
      await control("calibrate", current.generation);
      let s: Status;
      do {
        await new Promise(resolve => setTimeout(resolve, 250));
        s = await status();
      } while (s.phase === "calibrating");
      if (!s.calibrated || s.message !== "자동 보정 완료") return { error: s.message };
      applyStatus(s);
      config.mfish.cal_win = true;
      const applied = await setConfig({ mfish: structuredClone(config.mfish) });
      if ("error" in applied) return applied;
      return { mfish: { ...config.mfish }, done: "낚시 대기 창 · 미니게임 창" };
    }
    case "move_base":
      return moveTask("base");
    case "move_base_rev":
      return moveTask("baseRev");
    case "move_place":
      await configSaves; await loadNativeConfig(); await configureDesktop();
      await invoke("desktop_move_place", { place: Number(payload.place) });
      return { ok: true };
    case "move_record":
      await configSaves; await loadNativeConfig(); await configureDesktop();
      await invoke("desktop_record", { place: Number(payload.place) });
      return { ok: true };
    case "move_route_clear": {
      await configSaves; await loadNativeConfig();
      await invoke("desktop_route_clear", { place: Number(payload.place) });
      Object.assign(config, normalize(await invoke("macro_settings"))); persist();
      return { move: structuredClone(config.move) };
    }
    case "sell_test":
      return moveTask("sell");
    case "move_pick":
      return waypoint(payload, false);
    case "move_test":
      return waypoint(payload, true);
    case "move_get":
      await configSaves; await loadNativeConfig();
      Object.assign(config, normalize(await invoke("macro_settings"))); persist();
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
    case "mpos_point": case "mpos_region": {
      const feat = String(payload.feat);
      const key = String(payload.key);
      if (!["base", "mfish", "mmerch", "mcraft", "mmatch"].includes(feat) || !/^[a-z_]+$/.test(key)) {
        return { error: "저장할 위치를 확인하세요." };
      }
      await configSaves; await loadNativeConfig();
      const pos = await invoke<number[]>(name === "mpos_region" ? "desktop_pick_region" : "desktop_pick");
      const group = structuredClone(config[feat]) as Bag;
      group[key] = pos;
      const applied = await setConfig({ [feat]: group });
      if ("error" in applied) return applied;
      return { [feat]: structuredClone(config[feat]), pos };
    }
    case "play_pos": case "move_quest_pick": {
      await configSaves; await loadNativeConfig();
      const feat = name === "play_pos" ? "play" : "move";
      const key = feat === "play" ? payload.which === "play" ? "pos" : payload.which === "skip" ? "skip_pos" : "" : "quest_exit_pos";
      if (!key) return { error: "저장할 위치를 확인하세요." };
      const pos = await invoke<Position>("desktop_pick");
      const group = { ...(config[feat] as Bag), [key]: pos };
      const applied = await setConfig({ [feat]: group });
      if ("error" in applied) return applied;
      return { [feat]: structuredClone(config[feat]), pos };
    }
    case "mfish_check":
      return (await status()).calibrated ? { bar: "보정됨" } : { error: "자동 보정을 먼저 실행하세요." };
    default: {
      const error = "아직 이식되지 않은 기능입니다.";
      reportError(error);
      return { error };
    }
  }
}

const desktopWindow = window as Window & { acruxDesktopApi?: typeof desktopApi };
desktopWindow.acruxDesktopApi = desktopApi;
const script = document.createElement("script");
script.src = new URL("app.js", location.href).href;
script.addEventListener("load", addConnectionRow);
document.body.append(script);
