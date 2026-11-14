import { invoke } from "@tauri-apps/api/core";
import { biomeStatus, saveBiome, type BiomeSettings } from "./biome";
import defaults from "./desktop-defaults.json";
import { control, status, type SessionStatus, type Status, type Tuning } from "./engine";
import "./dua";
import { PortableMods, pickModPackage, type ModInfo } from "./portable-mods";

const key = "acrux.desktop.preferences";
type Bag = Record<string, unknown>;
type Position = [number, number];
type Waypoint = { pos: Position | null; time: number | null };
type Location = { name: string; feat: string; key: string; points: Waypoint[] };
type Movement = Bag & { places: Location[] };
interface Config extends Bag {
  macro_on: boolean;
  mfish: Bag;
  base: Bag;
  move: Movement;
}

function load(): Config {
  let saved = {};
  try { saved = JSON.parse(localStorage.getItem(key) ?? "{}") ?? {}; } catch {}
  const config: Config = { ...structuredClone(defaults), ...saved, macro_on: false };
  for (const [key, value] of Object.entries(defaults)) {
    if (value && typeof value === "object" && !Array.isArray(value)) {
      config[key] = { ...structuredClone(value), ...(config[key] as Bag ?? {}) };
    }
  }
  removeRetiredSettings(config);
  if (!["biome", "macro", "extra", "acrux"].includes(String(config.menu_tab))) config.menu_tab = "macro";
  for (const [key, name, feat, count] of [
    ["fish_spot", "낚시 장소", "mfish", 1], ["sell_spot", "물고기 판매 장소", "mfish", 1],
    ["craft_spot", "포션 제작 장소", "mcraft", 2], ["match_spot", "메모리 매치 장소", "mmatch", 1],
  ] as const) {
    if (!config.move.places.some(place => place.key === key)) {
      config.move.places.push({ key, name, feat, points: Array.from({ length: count }, () => ({ pos: null, time: null })) });
    }
  }
  try { localStorage.setItem(key, JSON.stringify(config)); } catch (error) { console.warn("설정 정리 저장 실패", error); }
  return config;
}

function removeRetiredSettings(config: Config) {
  // Discard retired feature settings while preserving inventory coordinates used by other macros.
  const previousItems = config.pop as Bag;
  for (const key of ["inventory_pos", "items_pos", "search_pos", "item_pos", "amount_pos", "use_pos", "ocr_region"]) {
    if (!config.base[key] && previousItems[key]) config.base[key] = previousItems[key];
  }
  const { after_join: _retiredJoinDelay, ...delays } = previousItems.delays as Bag ?? {};
  config.pop = { match_threshold: previousItems.match_threshold ?? 70, delays };
  const recovery = config.ret as Bag;
  config.ret = { ps_link: recovery.ps_link ?? "", start_wait: recovery.start_wait ?? 7.5 };
  for (const key of [
    "auto_restart_discord", "autostart", "beep", "channel_ids", "channels", "discord", "discord_connect",
    "enabled", "guild_ids", "ignore_words", "keywords", "max_age_sec", "names", "open_link", "prime",
    "show_all_links", "snipe", "sniping", "stable_join", "ai", "print_all_links", "run_on_link",
    "cooldown_sec", "kill_multiscope", "popping", "debug_port", "player_label",
  ]) delete config[key];
  for (const key of ["mods", "mod_settings"]) {
    if (config[key] && typeof config[key] === "object") {
      const { sniping: _retiredMod, ...settings } = config[key] as Bag;
      config[key] = settings;
    }
  }
}

const config = load();
const portableMods = new PortableMods(service);
window.AcruxPortableMods = { render: target => portableMods.render(target) };
const saved = () => ({ config: structuredClone(config) });
const persist = () => {
  removeRetiredSettings(config);
  localStorage.setItem(key, JSON.stringify({ ...config, macro_on: false }));
};

const stopped = ["stopped", "중지됨"];
const featureKeys = ["mpop", "mfish", "mitem", "mmerch", "mcraft", "mmatch", "mblood"] as const;
const enabledFeatures = () => featureKeys.filter(key => (config[key] as Bag).enabled === true);
const desktopStatus = (s: Status) => s.dua.running ? ["flux", s.dua.reason] : stopped;
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
  void invoke("service_action", { action: "diagnostic", payload: { message } }).catch(() => {});
  window.dispatchEvent(new CustomEvent("acrux-desktop-error", { detail: message }));
}

function snapshotPixels(buffer: ArrayBuffer) {
  if (!(buffer instanceof ArrayBuffer) || buffer.byteLength < 8) throw new Error("게임 화면 데이터를 읽을 수 없습니다.");
  const header = new DataView(buffer);
  const width = header.getUint32(0, true), height = header.getUint32(4, true);
  const bytes = width * height * 4;
  if (width < 1 || height < 1 || width > 16384 || height > 16384 || bytes > 256 * 1024 * 1024 || buffer.byteLength !== 8 + bytes) {
    throw new Error("게임 화면 크기나 픽셀 데이터가 올바르지 않습니다.");
  }
  return { width, height, pixels: new Uint8ClampedArray(buffer, 8) };
}

type ManualEvent = { kind: "key"; vk: number; down: boolean }
  | { kind: "pointer"; x: number; y: number; right: boolean; down: boolean }
  | { kind: "look"; dx: number; dy: number } | { kind: "release" };

function gameKey(code: string): number | undefined {
  if (/^Key[A-Z]$/.test(code)) return code.charCodeAt(3);
  if (/^Digit[0-9]$/.test(code)) return code.charCodeAt(5);
  return ({ Space: 0x20, ArrowLeft: 0x25, ArrowUp: 0x26, ArrowRight: 0x27, ArrowDown: 0x28,
    Escape: 0x1b, Enter: 0x0d, Tab: 0x09, Backspace: 0x08,
    ShiftLeft: 0x10, ShiftRight: 0x10, ControlLeft: 0x11, ControlRight: 0x11,
    AltLeft: 0x12, AltRight: 0x12 } as Record<string, number>)[code];
}

let closeGameView: (() => void) | undefined;
let releaseGameInput: (() => Promise<void>) | undefined;
let openingGameView = false;
let gameViewRevision = 0;
let gameInputCleanup = Promise.resolve();

async function openGameView() {
  if (closeGameView || openingGameView) return;
  openingGameView = true;
  const revision = gameViewRevision;
  try {
    await gameInputCleanup;
    let current = await status();
    if (revision !== gameViewRevision) throw new Error("화면 열기를 취소했습니다.");
    if (!current.captureReady || !current.inputReady || current.session.state !== "ready") throw new Error("[게임 열기]로 Roblox를 연결한 뒤 다시 시도하세요.");
    const dialog = document.createElement("dialog");
    dialog.id = "desktopGameView";
    dialog.className = "card";
    dialog.setAttribute("aria-labelledby", "desktopGameTitle");
    Object.assign(dialog.style, { maxWidth: "94vw", maxHeight: "94vh", background: "var(--panel)", color: "var(--fg)", border: "1px solid var(--line)", padding: "18px 20px" });
    const title = document.createElement("h3");
    title.id = "desktopGameTitle";
    title.textContent = "앱 전용 게임 화면";
    const hint = document.createElement("p");
    hint.className = "hint";
    const canvas = document.createElement("canvas");
    canvas.tabIndex = 0;
    canvas.setAttribute("aria-label", "앱 전용 게임 조작 화면");
    Object.assign(canvas.style, { display: "block", maxWidth: "min(88vw, 1200px)", maxHeight: "70vh", width: "auto", height: "auto", touchAction: "none" });
    const context = canvas.getContext("2d");
    if (!context) throw new Error("게임 화면을 표시할 수 없습니다.");
    const buttons = document.createElement("div");
    buttons.className = "confirm-btns";
    const close = document.createElement("button");
    close.type = "button";
    close.className = "btn ghost";
    close.textContent = "화면 닫기";
    buttons.append(close);
    dialog.append(title, hint, canvas, buttons);
    let closed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let lookTimer: ReturnType<typeof setTimeout> | undefined;
    type PendingInput = { event: ManualEvent; generation: number; revision: number };
    let critical: PendingInput[] = [];
    let motion: PendingInput | undefined;
    let pendingRelease: PendingInput | undefined;
    let releasePromise: Promise<void> | undefined;
    let resolveRelease: (() => void) | undefined;
    let pumping = false;
    const keys = new Map<string, number>(), mouse = new Set<number>();
    let inputRevision = 0;
    let pointer: Position = [0.5, 0.5];
    let lastPointer: Position | undefined;
    let look: Position = [0, 0];
    const manualAllowed = () => !closed && current.platform !== "windows" && current.captureReady && current.inputReady && current.session.state === "ready"
      && !config.macro_on && !current.dua.running && current.phase === "idle";
    const modeHint = () => { hint.textContent = current.platform === "windows" ? "Windows에서는 Roblox 창에서 직접 조작하세요. 이 화면은 미리보기입니다." : manualAllowed()
      ? "화면을 클릭한 뒤 키보드로 조작하세요. 우클릭 드래그로 시점을 움직입니다."
      : "매크로 실행 중에는 보기만 가능합니다. 조작하려면 매크로를 중지하세요."; };
    const clearHeld = () => {
      ++inputRevision;
      keys.clear(); mouse.clear(); lastPointer = undefined; look = [0, 0];
      if (lookTimer !== undefined) clearTimeout(lookTimer);
      lookTimer = undefined;
    };
    const pending = (event: ManualEvent): PendingInput => ({ event, generation: current.generation, revision: inputRevision });
    const pump = async () => {
      if (pumping) return;
      pumping = true;
      try {
        while (pendingRelease || critical.length || motion) {
          let next: PendingInput;
          if (pendingRelease) { next = pendingRelease; pendingRelease = undefined; }
          else if (critical.length) next = critical.shift()!;
          else { next = motion!; motion = undefined; }
          if (next.event.kind !== "release" && (next.revision !== inputRevision || !manualAllowed() || next.generation !== current.generation)) continue;
          try {
            await invoke("desktop_manual", { event: next.event, generation: next.generation });
          } catch (error) {
            if (!closed) hint.textContent = String(error);
            if (next.event.kind !== "release") void release();
            else { clearHeld(); critical = []; motion = undefined; }
          } finally {
            if (next.event.kind === "release") {
              const done = resolveRelease;
              releasePromise = undefined; resolveRelease = undefined;
              done?.();
            }
          }
        }
      } finally { pumping = false; }
    };
    const release = (): Promise<void> => {
      clearHeld(); critical = []; motion = undefined;
      if (!releasePromise) {
        releasePromise = new Promise<void>(resolve => { resolveRelease = resolve; });
        pendingRelease = pending({ kind: "release" });
      }
      void pump();
      return releasePromise;
    };
    const send = (event: ManualEvent) => {
      if (critical.length >= 64) {
        hint.textContent = "입력이 밀려 모두 해제했습니다. 다시 조작하세요.";
        void release();
        return;
      }
      critical.push(pending(event));
      void pump();
    };
    const sendMotion = (event: ManualEvent) => {
      if (event.kind === "look" && motion?.event.kind === "look") {
        event = { kind: "look", dx: event.dx + motion.event.dx, dy: event.dy + motion.event.dy };
      }
      if (event.kind === "look") {
        const limitX = Math.min(2048, canvas.width), limitY = Math.min(2048, canvas.height);
        event = { kind: "look", dx: Math.max(-limitX, Math.min(limitX, event.dx)), dy: Math.max(-limitY, Math.min(limitY, event.dy)) };
      }
      motion = pending(event);
      void pump();
    };
    releaseGameInput = release;
    const finish = () => {
      if (closed) return;
      closed = true;
      ++gameViewRevision;
      if (timer !== undefined) clearTimeout(timer);
      gameInputCleanup = release();
      window.removeEventListener("blur", blurred);
      closeGameView = undefined;
      releaseGameInput = undefined;
      dialog.close(); dialog.remove();
    };
    const blurred = () => { void release(); };
    closeGameView = finish;
    close.addEventListener("click", finish);
    dialog.addEventListener("cancel", event => { event.preventDefault(); finish(); });
    dialog.addEventListener("close", finish);
    dialog.addEventListener("keydown", event => event.stopPropagation());
    dialog.addEventListener("keyup", event => event.stopPropagation());
    window.addEventListener("blur", blurred);
    canvas.addEventListener("blur", blurred);
    canvas.addEventListener("contextmenu", event => event.preventDefault());
    const keyEvent = (event: KeyboardEvent, down: boolean) => {
      event.preventDefault(); event.stopPropagation();
      const vk = gameKey(event.code);
      if (vk === undefined) return;
      if (!manualAllowed()) { if (!down) void release(); return; }
      if (down && keys.has(event.code) || !down && !keys.has(event.code)) return;
      const alreadyHeld = [...keys.values()].includes(vk);
      if (down) keys.set(event.code, vk); else keys.delete(event.code);
      if (down && alreadyHeld || !down && [...keys.values()].includes(vk)) return;
      void send({ kind: "key", vk, down });
    };
    canvas.addEventListener("keydown", event => keyEvent(event, true));
    canvas.addEventListener("keyup", event => keyEvent(event, false));
    const position = (event: PointerEvent): Position | undefined => {
      const rect = canvas.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return;
      const x = (event.clientX - rect.left) / rect.width, y = (event.clientY - rect.top) / rect.height;
      if (x < 0 || x >= 1 || y < 0 || y >= 1 || !Number.isFinite(x + y)) return;
      return [x, y];
    };
    canvas.addEventListener("pointerdown", event => {
      event.preventDefault(); event.stopPropagation();
      if (!manualAllowed() || event.button !== 0 && event.button !== 2) return;
      const point = position(event);
      if (!point) return;
      canvas.focus(); canvas.setPointerCapture?.(event.pointerId);
      pointer = point; lastPointer = [event.clientX, event.clientY]; mouse.add(event.button);
      void send({ kind: "pointer", x: point[0], y: point[1], right: event.button === 2, down: true });
    });
    canvas.addEventListener("pointerup", event => {
      event.preventDefault(); event.stopPropagation();
      if (!mouse.delete(event.button)) return;
      motion = undefined; look = [0, 0];
      if (lookTimer !== undefined) clearTimeout(lookTimer);
      lookTimer = undefined;
      pointer = position(event) ?? pointer;
      if (!manualAllowed()) { void release(); return; }
      void send({ kind: "pointer", x: pointer[0], y: pointer[1], right: event.button === 2, down: false });
      if (!mouse.size) lastPointer = undefined;
    });
    canvas.addEventListener("pointercancel", () => { void release(); });
    canvas.addEventListener("pointermove", event => {
      if (!manualAllowed() || !mouse.size) return;
      event.preventDefault(); event.stopPropagation();
      pointer = position(event) ?? pointer;
      if (mouse.has(2) && lastPointer) {
        const rect = canvas.getBoundingClientRect();
        if (rect.width > 0 && rect.height > 0) {
          look[0] = Math.max(-2048, Math.min(2048, look[0] + (event.clientX - lastPointer[0]) * canvas.width / rect.width));
          look[1] = Math.max(-2048, Math.min(2048, look[1] + (event.clientY - lastPointer[1]) * canvas.height / rect.height));
        }
        lastPointer = [event.clientX, event.clientY];
      }
      if (lookTimer !== undefined) return;
      lookTimer = setTimeout(() => {
        lookTimer = undefined;
        if (!manualAllowed()) return;
        if (mouse.has(2)) {
          const dx = Math.round(look[0]), dy = Math.round(look[1]);
          look = [0, 0];
          if (dx || dy) sendMotion({ kind: "look", dx, dy });
        } else if (mouse.has(0)) sendMotion({ kind: "pointer", x: pointer[0], y: pointer[1], right: false, down: true });
      }, 34);
    });
    const refresh = async () => {
      try {
        const next = await status();
        if (closed) return;
        const changed = current.generation !== next.generation || current.dua.running !== next.dua.running;
        current = next;
        if (changed) await release();
        modeHint();
        if (!current.captureReady || !current.inputReady || current.session.state !== "ready") throw new Error(current.session.message);
        const image = snapshotPixels(await invoke<ArrayBuffer>("desktop_snapshot"));
        if (closed) return;
        canvas.width = image.width; canvas.height = image.height;
        context.putImageData(new ImageData(image.pixels, image.width, image.height), 0, 0);
      } catch (error) { if (!closed) { hint.textContent = String(error); await release(); } }
      finally { if (!closed) timer = setTimeout(() => { void refresh(); }, 500); }
    };
    modeHint();
    document.body.append(dialog); dialog.showModal(); canvas.focus();
    void refresh();
  } finally { openingGameView = false; }
}

let pickRevision = 0;
let pickingDesktop = false;
let pickingLive = false;
let cancelPreview: (() => void) | undefined;

async function cancelDesktopPick() {
  ++pickRevision;
  cancelPreview?.();
  if (pickingLive) await invoke("desktop_cancel_pick");
}

async function pickDesktop(): Promise<Position> {
  if (pickingDesktop) throw new Error("위치를 이미 지정 중입니다.");
  pickingDesktop = true;
  const revision = ++pickRevision;
  try {
    ++gameViewRevision;
    closeGameView?.();
    await gameInputCleanup;
    const current = await status();
    if (revision !== pickRevision) throw new Error("위치 지정을 취소했습니다.");
    if (!current.captureReady || !current.inputReady || current.session.state !== "ready") throw new Error("[게임 열기]로 Roblox를 연결한 뒤 다시 시도하세요.");
    if (current.dua.running || current.phase !== "idle") throw new Error("실행 중인 작업을 먼저 중지하세요.");
    const buffer = await invoke<ArrayBuffer>("desktop_snapshot");
    if (revision !== pickRevision) throw new Error("위치 지정을 취소했습니다.");
    const { width, height, pixels } = snapshotPixels(buffer);
    const dialog = document.createElement("dialog");
    dialog.id = "desktopPickPreview";
    dialog.className = "card";
    dialog.setAttribute("aria-labelledby", "desktopPickTitle");
    Object.assign(dialog.style, {
      maxWidth: "94vw", maxHeight: "94vh", background: "var(--panel)", color: "var(--fg)",
      border: "1px solid var(--line)", padding: "18px 20px",
    });
    const title = document.createElement("h3");
    title.id = "desktopPickTitle";
    title.textContent = "게임 화면에서 저장할 위치를 클릭하세요.";
    const hint = document.createElement("p");
    hint.className = "hint";
    hint.textContent = "캡처한 게임 화면입니다. Esc 또는 취소로 닫을 수 있습니다.";
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    canvas.setAttribute("aria-label", "게임 위치 선택 화면");
    Object.assign(canvas.style, {
      display: "block", maxWidth: "min(88vw, 1200px)", maxHeight: "70vh", width: "auto", height: "auto", cursor: "crosshair",
    });
    const context = canvas.getContext("2d");
    if (!context) throw new Error("게임 화면을 표시할 수 없습니다.");
    context.putImageData(new ImageData(pixels, width, height), 0, 0);
    const buttons = document.createElement("div");
    buttons.className = "confirm-btns";
    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "btn ghost";
    cancel.textContent = "취소";
    buttons.append(cancel);
    dialog.append(title, hint, canvas, buttons);
    return await new Promise<Position>((resolve, reject) => {
      let settled = false;
      const finish = (point?: Position) => {
        if (settled) return;
        settled = true;
        cancelPreview = undefined;
        dialog.close();
        dialog.remove();
        if (point) resolve(point);
        else reject(new Error("위치 지정을 취소했습니다."));
      };
      cancelPreview = () => finish();
      cancel.addEventListener("click", () => finish());
      dialog.addEventListener("cancel", event => { event.preventDefault(); finish(); });
      dialog.addEventListener("close", () => finish());
      dialog.addEventListener("keydown", event => {
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); finish(); }
      });
      canvas.addEventListener("click", event => {
        const rect = canvas.getBoundingClientRect();
        if (rect.width <= 0 || rect.height <= 0) return;
        const x = (event.clientX - rect.left) / rect.width;
        const y = (event.clientY - rect.top) / rect.height;
        if (Number.isFinite(x) && Number.isFinite(y) && x >= 0 && y >= 0 && x < 1 && y < 1) finish([x, y]);
      });
      document.body.append(dialog);
      dialog.showModal();
      cancel.focus();
    });
  } finally {
    cancelPreview?.();
    pickingDesktop = false;
  }
}

let desktopRevision = 0;
let desiredMacro = false;
let desktopPending = 0;
async function syncDesktop(revision = desktopRevision) {
  const current = await status();
  if (revision !== desktopRevision || !config.macro_on) return;
  if (current.dua.running) return;
  if (!current.captureReady || !current.inputReady || current.session.state !== "ready") throw new Error("[게임 열기]로 Roblox를 연결한 뒤 다시 시도하세요.");
  await releaseGameInput?.();
  if (revision !== desktopRevision || !config.macro_on) return;
  await invoke<Status>("desktop_start", { features: enabledFeatures(), generation: current.generation });
}

let configLoaded: Promise<void> | undefined;
async function loadNativeConfig() {
  if (!configLoaded) configLoaded = (async () => {
    const native = await invoke<Config>("macro_settings");
    if (!native || typeof native !== "object" || Array.isArray(native)) throw new Error("저장된 매크로 설정을 읽을 수 없습니다.");
    for (const [group, fallback] of Object.entries(defaults)) {
      if (fallback && typeof fallback === "object" && !Array.isArray(fallback)) {
        native[group] = { ...structuredClone(fallback), ...(native[group] as Bag ?? {}) };
      }
    }
    Object.assign(config, native, { macro_on: false });
    removeRetiredSettings(config);
    persist();
  })().catch(error => { configLoaded = undefined; throw error; });
  return configLoaded;
}
let configSaves = Promise.resolve();
async function saveNativeConfig(candidate: Config = config) {
  const running = config.macro_on;
  const native = await invoke<Config>("save_macro_settings", { config: { ...structuredClone(candidate), macro_on: false } });
  Object.assign(config, native, { macro_on: running && config.macro_on });
  persist();
}
async function nativeAction(action: string, payload: Bag = {}) {
  const current = await status();
  return invoke<Bag>("automation_action", { action, payload, generation: current.generation });
}
async function service(action: string, payload: Bag = {}) {
  const run = async () => {
    const result = await invoke<Bag>("service_action", { action, payload });
    if (action === "mods_settings") {
      const all = config.mod_settings as Bag ?? {};
      config.mod_settings = { ...all, [String(payload.id)]: result.settings };
      persist();
    }
    return result;
  };
  if (action !== "mods_settings") return run();
  const pending = configSaves.then(run);
  configSaves = pending.then(() => undefined, () => undefined);
  return pending;
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
  if (!patch || typeof patch !== "object" || Array.isArray(patch)) throw new Error("저장할 설정을 확인하세요.");
  // Stop is deliberately outside the save queue so it can invalidate a pending start.
  if ("macro_on" in patch) desiredMacro = patch.macro_on === true;
  if (patch.macro_on === false) {
    ++desktopRevision;
    config.macro_on = false;
    await cancelDesktopPick();
    await control("stop");
    persist();
    if (Object.keys(patch).length === 1) return saved();
  }
  const revision = ++desktopRevision;
  desktopPending++;
  const save = configSaves.then(async () => {
    await loadNativeConfig();
    if (patch.biome) await saveBiome(biomeSettings(patch.biome as Bag));
    if ("online_share" in patch) await invoke("save_online", { share: patch.online_share !== false });
    const previous = JSON.stringify({ ...config, macro_on: false });
    const next = { ...structuredClone(config), ...structuredClone(patch), macro_on: false } as Config;
    removeRetiredSettings(next);
    const changed = previous !== JSON.stringify(next);
    const current = await status();
    if (changed && (current.dua.running || current.phase !== "idle")) await control("stop");
    if (changed) await saveNativeConfig(next);
    config.macro_on = desiredMacro;
    if (revision === desktopRevision && config.macro_on) await syncDesktop(revision);
    persist();
    return saved();
  });
  configSaves = save.then(() => undefined, () => undefined);
  try { return await save; } catch (error) {
    if (revision === desktopRevision) { desiredMacro = false; config.macro_on = false; persist(); reportError(String(error)); }
    return { ...saved(), error: error instanceof Error ? error.message : String(error) };
  } finally { desktopPending--; }
}

async function pickFile() {
  return pickModPackage(service);
}

async function pickSetting(feat: string, key: string, region: boolean) {
  await configSaves;
  await loadNativeConfig();
  if (!Object.hasOwn(config, feat) || !["base", "mfish", "mmerch", "mcraft", "mmatch", "play", "move"].includes(feat)
      || !/^[a-z_]+$/.test(key)) throw new Error("저장할 위치를 확인하세요.");
  const current = await status();
  if (current.platform === "windows") {
    if (pickingDesktop) throw new Error("위치를 이미 지정 중입니다.");
    if (!current.captureReady || !current.inputReady || current.session.state !== "ready") throw new Error("[게임 열기]로 Roblox를 연결한 뒤 다시 시도하세요.");
    if (current.dua.running || current.phase !== "idle") throw new Error("실행 중인 작업을 먼저 중지하세요.");
    pickingDesktop = true;
    const revision = ++pickRevision;
    try {
      ++gameViewRevision;
      closeGameView?.();
      await gameInputCleanup;
      if (revision !== pickRevision) throw new Error("위치 지정을 취소했습니다.");
      pickingLive = true;
      const result = await invoke<{ pos: number[]; config: Config }>("pick_macro_position", { feat, key, region, generation: current.generation });
      if (revision !== pickRevision) throw new Error("위치 지정을 취소했습니다.");
      Object.assign(config, result.config, { macro_on: config.macro_on });
      persist();
      return { [feat]: structuredClone(config[feat]), pos: result.pos };
    } finally { pickingLive = false; pickingDesktop = false; }
  }
  const first = await pickDesktop();
  let pos: number[] = first;
  if (region) {
    const second = await pickDesktop();
    pos = [Math.min(first[0], second[0]), Math.min(first[1], second[1]), Math.max(first[0], second[0]), Math.max(first[1], second[1])];
    if (pos[0] === pos[2] || pos[1] === pos[3]) throw new Error("영역의 서로 다른 두 모서리를 고르세요.");
  }
  const candidate = structuredClone(config);
  const group = candidate[feat] as Bag;
  group[key] = pos;
  await saveNativeConfig(candidate);
  return { [feat]: structuredClone(group), pos };
}

const calibrations = new Set<string>();
async function calibration(action: string, group: string) {
  if (calibrations.has(action)) {
    await nativeAction(group + "_stop");
    throw new Error("자동 보정 취소");
  }
  const before = await status();
  if (before.dua.running || before.phase !== "idle") throw new Error("실행 중인 작업을 먼저 중지하세요.");
  calibrations.add(action);
  try {
    const result = await nativeAction(action);
    const baseline = result.automation as Bag;
    const generation = result.generation;
    for (let i = 0; i < 480; i++) {
      await new Promise(resolve => setTimeout(resolve, 250));
      const s = await status() as Status & { automation?: Bag };
      const a = s.automation;
      if (s.generation !== generation) throw new Error("자동 보정 요청이 취소됐습니다.");
      if (!a) throw new Error("자동 보정 상태를 읽을 수 없습니다.");
      if (Number(a.failed ?? 0) > Number(baseline?.failed ?? 0)) throw new Error(String(a.message || s.message));
      if (Number(a.completed ?? 0) > Number(baseline?.completed ?? 0)) {
        const next = await invoke<Config>("macro_settings");
        Object.assign(config, next, { macro_on: config.macro_on });
        persist();
        return { ...result, [group]: structuredClone(config[group]), done: a.message };
      }
      if (!a.running && !a.active && !a.queued) throw new Error(String(a.message || s.message || "자동 보정이 중지됐습니다."));
    }
    await nativeAction(group + "_stop");
    throw new Error("자동 보정 시간이 초과되었습니다. 게임 화면과 기록을 확인하세요.");
  } finally { calibrations.delete(action); }
}

let last = "";
let seq = 0;
let haveMods = false;
const seenNativeLogs = new Set<string>();
const openedAt = Date.now() / 1000;
async function hook(event: string, args: unknown[] = []) {
  return haveMods ? service("mods_hook", { event, args }) : {} as Bag;
}
function numberStats(value: unknown): Record<string, number> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  return Object.fromEntries(Object.entries(value).filter(([, value]) => typeof value === "number" && Number.isFinite(value)).map(([key, value]) => [key, Math.trunc(value as number)]));
}
async function desktopApi(name: string, payload: Bag) {
  try { return await request(name, payload); }
  catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (name === "state") return { ...saved(), version: "0.1.0", status: stopped, mods: [], error: message };
    if (name === "poll") return { error: message, status: stopped, macro_on: false,
      session: { supported: true, state: "error", message, width: 0, height: 0 } };
    return { error: message };
  }
}

async function request(name: string, payload: Bag) {
  switch (name) {
    case "state": {
      const errors: string[] = [];
      const attempt = async <T>(label: string, run: () => Promise<T>, fallback: T): Promise<T> => {
        try { return await run(); } catch (error) { errors.push(`${label}: ${String(error)}`); reportError(`${label}: ${String(error)}`); return fallback; }
      };
      await attempt("설정", loadNativeConfig, undefined);
      const [s, mods, biome, online] = await Promise.all([
        attempt("게임 상태", status, null),
        attempt<Bag>("모드", () => service("mods_list"), { mods: [] }),
        attempt("바이옴", biomeStatus, null),
        attempt("온라인", () => invoke<{ share: boolean }>("online_status"), null),
      ]);
      await portableMods.start((mods.mods ?? []) as ModInfo[]);
      mods.mods = portableMods.list((mods.mods ?? []) as ModInfo[]);
      haveMods = Array.isArray(mods.mods) && mods.mods.some((mod: Bag) => mod.runtime !== "javascript" && mod.loaded === true);
      if (biome) config.biome = { enabled: biome.settings.enabled, player: biome.settings.player, ps_link: biome.settings.psLink, webhooks: biome.settings.webhooks, everyone: biome.settings.mentions, roles: biome.settings.roles };
      if (online) config.online_share = online.share;
      if (s) applyStatus(s);
      const output = { ...saved(), version: s?.version ?? "0.1.0", status: s ? desktopStatus(s) : stopped,
        dua: s?.dua, session: s?.session, mods: mods.mods ?? [], mods_error: mods.error, error: errors.join("\n") || undefined };
      const changed = await attempt<Bag>("모드 상태", () => hook("state", [output]), {});
      return Array.isArray(changed.args) && changed.args[0] && typeof changed.args[0] === "object" ? changed.args[0] : output;
    }
    case "poll": {
      const s = await status();
      const biome = await biomeStatus().catch(() => ({ current: null, history: [], owner: null, watching: false }));
      const online = await invoke<{ online: number | null }>("online_status").catch(() => ({ online: null }));
      const automation = (s as Status & { automation?: Bag }).automation ?? {};
      const native = await service("runtime_metadata").catch(() => ({} as Bag));
      const active = automation.active as Bag | null;
      const activity = (kind: string) => ({ running: !!automation.running && active?.kind === kind, msg: String(automation.message ?? s.message) });
      const move = { ...activity("go"), running: s.phase === "moving" || active?.kind === "go", msg: s.message,
        remembered: s.rememberedDestinations ?? [], measuring: null };
      if (!s.dua.running && s.phase === "idle" && config.macro_on && !desktopPending) { desiredMacro = false; config.macro_on = false; }
      const message = s.dua.running ? `DUA · ${s.dua.route ?? "대기"} · ${s.dua.reason} · ${s.message}` : s.message;
      const logs = message === last ? [] : [{ seq: ++seq, time: Date.now() / 1000, msg: message, color: "d" }];
      last = message;
      for (const line of native.logs as { time: number; msg: string }[] ?? []) {
        const id = `${line.time}:${line.msg}`;
        if (!seenNativeLogs.has(id)) { logs.push({ seq: ++seq, time: line.time, msg: line.msg, color: "d" }); seenNativeLogs.add(id); }
      }
      while (seenNativeLogs.size > 600) seenNativeLogs.delete(seenNativeLogs.values().next().value!);
      const used = automation.used as Bag ?? {};
      const bought = Object.values(automation.bought as Bag ?? {}).reduce<number>((sum, value) => sum + Number(value), 0);
      const sessionStats: Record<string, number> = {
        fish_success: s.stats.success, fish_junk: s.stats.junk, fish_fail: s.stats.fail, fish_full: s.stats.full,
        item_strange: Number(used["Strange Controller"] ?? 0), item_randomizer: Number(used["Biome Randomizer"] ?? 0),
        blood_used: Number(used["Cursed Rune Fragment"] ?? 0), merch_bought: bought,
        crafted: Number(automation.crafted ?? 0), match_played: Number(automation.played ?? 0), match_ads: Number(automation.ads ?? 0),
      };
      const statsHook = await hook("stats").catch(() => ({} as Bag));
      for (const extra of statsHook.results as unknown[] ?? []) Object.assign(sessionStats, numberStats(extra));
      const output: Bag = {
        biome: { current: biome.current, history: biome.history, owner: biome.owner, log: biome.watching },
        online: online.online, online_on: true,
        seq, logs, status: desktopStatus(s), dua: s.dua, session: s.session,
        roblox: s.gameAvailable, macro_on: config.macro_on, move,
        mfish: { running: ["searching", "casting", "waiting", "reeling", "result", "selling"].includes(s.phase), msg: s.message, stats: s.stats },
        mpop: activity("pop"), mblood: { ...activity("blood"), used: sessionStats.blood_used },
        mitem: { ...activity("items"), items: Object.fromEntries(["strange", "randomizer"].map(key => [key, { on: (config.mitem as Bag)[key], used: key === "strange" ? sessionStats.item_strange : sessionStats.item_randomizer, left: Math.max(0, Number((automation.itemNext as Bag ?? {})[key] ?? 0) - Number(automation.clock ?? 0)) }])) },
        mmerch: { ...activity("merchant"), bought },
        mcraft: { ...activity("craft"), crafted: automation.crafted ?? 0 },
        mmatch: { ...activity("match"), played: automation.played ?? 0, ads: automation.ads ?? 0, next_in: Math.max(0, Number(automation.nextMatch ?? 0) - Number(automation.clock ?? 0)) },
        play: activity("play"), ret: activity("return"), automation,
        crash: automation.recoveryLeft == null ? null : { left: Number(automation.recoveryLeft) },
        stats: { session: sessionStats, all: { ...numberStats(native.stats), fish_success: s.totals.success, fish_junk: s.totals.junk, fish_fail: s.totals.fail, fish_full: s.totals.full }, since: (native.stats as Bag | undefined)?.since, started: openedAt },
      };
      if (haveMods) {
        const hooked = await hook("poll", [output, Number(payload.since ?? 0)]);
        if (Array.isArray(hooked.args) && hooked.args[0] && typeof hooked.args[0] === "object") return hooked.args[0];
      }
      return output;
    }
    case "mods_install":
      return payload.path || payload.bytes ? service(name, payload) : pickFile();
    case "mods_list": case "mods_toggle": case "mods_remove": {
      const result = await service(name, payload);
      return { ...result, mods: portableMods.list((result.mods ?? []) as ModInfo[]) };
    }
    case "mods_folder":
    case "open_web": case "open_folder": case "restart": case "changelog":
    case "update_state": case "update_check": case "update_download": case "update_install":
      return service(name, payload);
    case "uninstall":
      // The existing PC confirmation dialog calls this only after confirmYes.
      return service(name, { confirm: true });
    case "webhook_test":
      await invoke("biome_test");
      return { queued: true };
    case "discord_state":
      return invoke("discord_state");
    case "discord_login":
    case "discord_cancel":
    case "discord_logout": {
      const state = await invoke<{ code: string | null; error: string }>(name);
      return state.error ? { error: state.error } : { ok: true, code: state.code };
    }
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
    case "move_base": case "move_base_rev": case "move_place": case "move_test":
      return nativeAction(name, { ...payload, i: payload.place ?? payload.i });
    case "move_pick":
      return nativeAction("waypoint_mark", { destination: payload.destination ?? "coast" });
    case "move_arrive":
      return nativeAction("waypoint_cancel");
    case "move_quest_pick":
      return pickSetting("move", "quest_exit_pos", false);
    case "move_get":
      Object.assign(config, await invoke<Config>("macro_settings"), { macro_on: config.macro_on });
      return { move: structuredClone(config.move) };
    case "move_stop":
      ++desktopRevision;
      desiredMacro = false;
      config.macro_on = false;
      await cancelDesktopPick();
      await control("stop");
      return { ok: true };
    case "pick_cancel":
      await cancelDesktopPick();
      return {};
    case "session_start":
      return { session: await invoke<SessionStatus>("desktop_session_start") };
    case "session_stop":
      ++gameViewRevision;
      closeGameView?.();
      await gameInputCleanup;
      await cancelDesktopPick();
      ++desktopRevision;
      desiredMacro = false;
      config.macro_on = false;
      return { session: await invoke<SessionStatus>("desktop_session_stop") };
    case "game_view":
      await openGameView();
      return { ok: true };
    case "game_view_close":
      ++gameViewRevision;
      closeGameView?.();
      return {};
    case "mpos_point": case "mpos_region":
      return pickSetting(String(payload.feat), String(payload.key), name === "mpos_region");
    case "play_pos":
      return pickSetting("play", payload.which === "play" ? "pos" : payload.which === "skip" ? "skip_pos" : "", false);
    case "mfish_stop": {
      ++desktopRevision;
      desiredMacro = false;
      config.macro_on = false;
      const current = await status();
      if (current.dua.running || current.phase !== "calibrating") await control("stop");
      return {};
    }
    case "mfish_check":
      return nativeAction(name, payload);
    case "sell_autocal": return calibration(name, "mfish");
    case "mmerch_autocal": return calibration(name, "mmerch");
    case "mcraft_autocal": return calibration(name, "mcraft");
    case "mmatch_autocal": case "mmatch_boardcal": return calibration(name, "mmatch");
    case "mpop_default": {
      const result = await nativeAction(name, payload);
      ((config.mpop as Bag).templates as Bag)[String(payload.biome).toUpperCase()] = result.template;
      persist();
      return result;
    }
    case "mpop_test": case "mpop_stop": case "mpop_ocr_test":
    case "mitem_test": case "mitem_stop": case "mblood_test": case "mblood_stop":
    case "mmerch_check": case "mmerch_buy_test": case "mmerch_stop":
    case "mcraft_test": case "mcraft_stop": case "mmatch_test": case "mmatch_stop":
    case "go": case "waypoint_mark": case "waypoint_cancel":
      await configSaves;
      return nativeAction(name, payload);
    case "ret_join": {
      const current = await status();
      if (current.session.state !== "ready") return { session: await invoke<SessionStatus>("desktop_session_start") };
      return nativeAction(name, payload);
    }
    case "sell_test": case "ret_test": case "play_test": case "play_stop":
    case "crash_cancel": case "ocr_info": case "ocr_test":
      return nativeAction(name, payload);
    default:
      return service("mods_call", { name, payload });
  }
}

const desktopWindow = window as Window & { acruxDesktopApi?: typeof desktopApi };
desktopWindow.acruxDesktopApi = desktopApi;
const script = document.createElement("script");
script.src = new URL("app.js", location.href).href;
document.body.append(script);
