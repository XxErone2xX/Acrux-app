import { invoke } from "@tauri-apps/api/core";
import { duaStop, saveTuning, status, type Status, type Stats, type Tuning } from "./engine";
import mobileDictionary from "../android/app/src/main/assets/i18n/mobile.tsv?raw";
import desktopDictionary from "../android/app/src/main/assets/i18n/pc.tsv?raw";

import defaults from "./desktop-defaults.json";
import { biomeStatus, saveBiome, defaultBiome, type BiomeStatus } from "./biome";

type Language = "ko" | "en" | "ja";
type Page = "home" | "fish" | "biome" | "log" | "settings";
type Overlay = { visible: boolean; hideOnApp: boolean; alpha: number; lang: Language };
type Setup = "biome" | "accessibility" | "capture" | "stopCapture" | "appInfo" | "donate" | "overlay" | "inputSetup";
type Reset = "calibration" | "session" | "total" | "log";
type GuideId = "ready" | "fish" | "move" | "sell";

const translations = new Map<string, string[]>();
for (const dictionary of [desktopDictionary, mobileDictionary]) {
  for (const line of dictionary.split(/\r?\n/)) {
    if (!line || line.startsWith("#")) continue;
    const [ko, ...other] = line.split("\t");
    translations.set(ko.replaceAll("\\n", "\n"), other.map(value => value.replaceAll("\\n", "\n")));
  }
}

function restore<T>(key: string, fallback: T): T {
  try { return JSON.parse(localStorage.getItem(key) ?? "null") ?? fallback; }
  catch { return fallback; }
}

const prefsKey = "acrux.mobile.overlay";
const detected = navigator.language.toLowerCase().split("-")[0];
let overlay = restore<Overlay>(prefsKey, {
  visible: true, hideOnApp: true, alpha: 90,
  lang: detected === "en" || detected === "ja" ? detected : "ko",
});

function t(text: string) {
  if (overlay.lang === "ko") return text;
  return translations.get(text)?.[overlay.lang === "en" ? 0 : 1] || text;
}

function escape(text: string) {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

const tr = (text: string) => escape(t(text));
const element = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const tabs: readonly [Page, string][] = [["home", "홈"], ["fish", "낚시"], ["biome", "바이옴"], ["log", "기록"], ["settings", "설정"]];
type Bag = Record<string, unknown>;
const features = [["mfish", "자동 낚시"], ["mpop", "오토 팝핑"], ["mitem", "오토 아이템"], ["mmerch", "상인 자동 구매"], ["mcraft", "자동 포션 제작"], ["mmatch", "자동 메모리 매치"], ["mblood", "블러드 레인 자동 생성"]] as const;
let macro: Bag = structuredClone(defaults);
let macroLoaded = false;
let inputMethod = { enabled: false, selected: false };
let update: Bag = { available: false, ready: false, message: "확인 전" };
let macroSaves = Promise.resolve();
let macroRestartRevision: number | null = null;
let macroSaving = 0;
const group = (name: string) => macro[name] as Bag;
const selectedFeatures = () => features.filter(([name]) => group(name)?.enabled === true).map(([name]) => name);
async function loadMacro() {
  const next = await invoke<Bag>("macro_settings");
  macro = { ...structuredClone(defaults), ...next };
  for (const [key, value] of Object.entries(defaults)) if (value && typeof value === "object" && !Array.isArray(value)) macro[key] = { ...value, ...(next[key] as Bag ?? {}) };
  macroLoaded = true;
}
async function automationAction(action: string, payload: Bag = {}) {
  const observed = await status();
  return invoke<Bag>("automation_action", { action, payload, generation: observed.generation });
}
async function serviceAction(action: string, payload: Bag = {}) {
  return invoke<Bag>("service_action", { action, payload });
}
async function saveMacro(edit: (next: Bag) => void, tuning?: Partial<Tuning>) {
  ++request;
  ++macroSaving;
  const revision = macroRevision;
  const pending = macroSaves.then(async () => {
    if (!macroLoaded) await loadMacro();
    const next = structuredClone(macro);
    edit(next);
    const observed = await status();
    const restart = macroPending !== "stop" && (observed.dua.running || macroRestartRevision === revision) && revision === macroRevision;
    if (restart) { macroRestartRevision = revision; applyDua(); }
    if (observed.dua.running || observed.phase !== "idle") {
      const stopped = await duaStop();
      if (revision === macroRevision) current = stopped;
    }
    if (tuning) {
      const tuned = await saveTuning({ ...observed.tuning, ...tuning });
      if (revision === macroRevision) current = tuned;
    }
    next.macro_on = false;
    macro = await invoke<Bag>("save_macro_settings", { config: next });
    if (restart && revision === macroRevision && macroPending !== "stop") {
      const fresh = await status();
      if (revision === macroRevision) current = await invoke<Status>("dua_start", { features: selectedFeatures(), generation: fresh.generation });
    }
  });
  const settled = pending.finally(() => {
    --macroSaving;
    if (macroRestartRevision === revision) macroRestartRevision = null;
    applyStatus();
  });
  macroSaves = settled.catch(() => undefined);
  return settled;
}
const tuningNames: Record<keyof Tuning, string> = {
  biteSeconds: "bite_max", leadMs: "lead_ms", targetPercent: "target_pct", clickMs: "click_ms", clickGapMs: "click_gap_ms", castRetries: "cast_retry", deadband: "deadband", adaptive: "adaptive", tapAbove: "tap_above",
};
async function changeTuning(key: keyof Tuning, value: number | boolean) {
  await saveMacro(next => { (next.mfish as Bag)[tuningNames[key]] = value; }, { [key]: value });
}

const zero: Stats = { success: 0, junk: 0, fail: 0, full: 0 };
const pageMemory = new Map<Page, number>();
const tabKey = "acrux.mobile.tab";
const savedPage = restore<string>(tabKey, "home");
const lastPage = savedPage === "move" ? "fish" : savedPage as Page;
let page: Page = tabs.some(([id]) => id === lastPage) ? lastPage : "home";
let current: Status | undefined;
let biome: BiomeStatus = { settings: defaultBiome, current: null, watching: false, message: "꺼짐", logs: [], history: [], owner: "" };
let transport = { connection: "off", testOutput: "", testUntil: 0 };
let online: { online: number | null; share: boolean } = { online: null, share: true };
let busy = false;
let request = 0;
let toastTimer = 0;
let macroPending: "start" | "stop" | null = null;
let macroRevision = 0;
interface DiscordState { linked: boolean; name: string; code: string | null; error: string }
let discord: DiscordState = { linked: false, name: "", code: null, error: "" };
let discordBusy = false;
let discordLoaded = false;
let discordRevision = 0;
let discordTimer: ReturnType<typeof setTimeout> | undefined;

function message(text: string) {
  const toast = element("toast");
  toast.textContent = t(text);
  toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => { toast.hidden = true; }, 3000);
}

function button(title: string, action: string, color = "muted", id = "", disabled = false) {
  return `<button type="button" class="button ${color}" data-action="${action}" ${id ? `id="${id}"` : ""} ${disabled ? "disabled" : ""}>${tr(title)}</button>`;
}

function card(id: string, title: string, body: string) {
  return `<section id="${id}" class="card" data-title="${tr(title)}"><h2>${tr(title)}</h2>${body}</section>`;
}

function text(body: string, size = 13, tone = "muted") {
  return `<p class="${tone}" style="font-size:${size}px">${tr(body)}</p>`;
}

function row(title: string, description: string, right: string, stateId = "") {
  return `<div class="row"><div><strong>${tr(title)}</strong><small>${tr(description)}</small>${stateId ? `<p id="${stateId}" class="muted"></p>` : ""}</div>${right}</div>`;
}

function numberInput(title: string, desc: string, key: keyof Tuning | "", value: number, min: number, max: number, step = 1) {
  const field = `<input type="number" aria-label="${tr(title)}" value="${value}" min="${min}" max="${max}" step="${step}" ${key ? `data-tuning="${key}"` : "disabled"}>`;
  return row(title, desc, field);
}

function toggle(title: string, data: string, value = false, disabled = false) {
  return `<label class="toggle"><span>${tr(title)}</span><input type="checkbox" ${data} ${value ? "checked" : ""} ${disabled ? "disabled" : ""}><i></i></label>`;
}

const guideNames: Record<GuideId, string> = {
  ready: "시작 준비", fish: "낚시 보정", move: "이동 설정", sell: "판매 설정",
};

function home() {
  const languages: [Language, string][] = [["ko", "한국어"], ["en", "English"], ["ja", "日本語"]];
  return card("language", "언어",
    text("앱 언어 · 떠 있는 Acrux 창도 같이 바뀜", 12)
    + `<div class="buttons">${languages.map(([lang, title]) => `<button class="chip" data-action="lang:${lang}" aria-pressed="${lang === overlay.lang}">${title}</button>`).join("")}</div>`)
  + card("ready", "준비",
    row("① 접근성 권한", "Acrux가 로블록스 화면을 대신 터치함 · 설정 → 접근성 → 설치된 앱 → Acrux Mobile", button("켜기", "accessibility", "accent", "access-button"), "access-state")
    + `<button class="tip" data-action="appInfo">${tr("안드로이드 13 이상에서 '제한된 설정'이라며 켜지지 않으면: 설정 → 앱 → Acrux Mobile → 오른쪽 위 ⋮ → '제한된 설정 허용' 후 다시 켜기")}</button>`
    + row("② 화면 캡처", "매크로가 화면을 보고 상태를 판단함 · 녹화 · 저장은 하지 않음 · '전체 화면' 공유를 고르기", button("시작", "capture", "accent", "capture-button"), "capture-state")
    + row("③ 글자 입력", "검색어와 수량을 게임에 입력 · 기기에 따라 입력기 설정이 필요함", button("설정", "inputSetup", "accent", "input-button"), "input-state")
    + text("Roblox 화면에서만 터치 · 다른 앱에서는 대기", 12, "warning"))
  + card("state", "상태",
    '<p id="run-state" class="readout"></p><p id="cal-state" class="muted readout"></p>'
    + `<div class="buttons">${button("시작", "macro-toggle", "accent", "macro-toggle")}</div>`
    + `<div class="buttons">${button("보정 초기화", "reset:calibration", "danger")}</div>`)
  + card("features", "실행할 기능", features.map(([key, name]) => toggle(name, `data-macro="${key}.enabled"`, !!group(key)?.enabled)).join("")
    + text("기능별 설정과 위치 지정은 설정 칸에서 할 수 있습니다.", 12)
    + '<p id="automation-state" class="muted readout"></p>')
  + card("dua", "DUA 실행 경로",
    '<p id="dua-state" class="readout"></p><p id="dua-reason" class="muted readout"></p><p id="dua-counts" class="muted readout"></p><p id="dua-guard" class="warning readout"></p>'
    + text("상태와 명령을 우선 사용하고, 화면 확인이 필요할 때 캡처", 12))
  + card("how", "사용법",
    [
      "1. 홈에서 실행할 기능을 켜고 로블록스 (Sol's RNG)를 가로 전체 화면으로 열기",
      "2. [시작] — 이동 설정에 따라 목적지를 찾고, 낚시는 실제 패널을 보정한 뒤 진행",
      "3. Roblox가 앞에 있을 때만 터치 · 다른 앱에서는 대기 · 잠기면 정지",
      "4. Acrux 창은 어디든 잡고 끌어서 낚시 창 · 결과창을 가리지 않는 곳으로 옮기기",
      "5. 멈출 때는 앱 또는 떠 있는 Acrux 창의 [정지] · 이동은 매번 지형을 확인하고 판매 후 돌아옴",
      "6. 바이옴 알림: 바이옴 칸 → 무선 디버깅 연결 → 웹후크 입력 → Acrux 창의 [바이옴] 버튼으로 켜고 끄기",
    ].map(step => text(step)).join("")
    + `<strong class="guide-heading">${tr("설정 안내 — 기능별로 하나씩 따라 하기 (PC 튜토리얼과 같음)")}</strong><div class="guide-buttons">${Object.entries(guideNames).map(([id, name]) => button(name, "guide:" + id, "accent")).join("")}</div>`)
  + card("donate", "후원", text("Acrux 가 도움이 됐다면 후원으로 응원해 주세요", 12)
    + `<div class="buttons">${button("후원하기", "donate", "gold")}</div>`);
}

function fishing() {
  const names = ["성공", "쓰레기", "실패", "성공률"];
  const stats = `<table><thead><tr><td></td><th>${tr("이번 실행")}</th><th>${tr("전체")}</th></tr></thead><tbody>${names.map((name, i) =>
    `<tr><th scope="row">${tr(name)}</th><td id="run-${i}"></td><td id="total-${i}"></td></tr>`).join("")}</tbody></table>`;
  const cfg = current?.tuning;
  return card("statistics", "낚시 통계",
    stats + `<div class="buttons">${button("이번 실행 초기화", "reset:session")}${button("전체 초기화", "reset:total", "danger")}</div>`)
  + card("fishing-options", "세부 설정",
    numberInput("입질 최대 대기 (초)", "넘으면 Exit 후 다시 던짐 · 기본 60", "biteSeconds", cfg?.biteSeconds ?? 60, 5, 600)
    + numberInput("누르는 시간 (ms)", "릴링 때 한 번 누르는 시간 · 기본 30", "clickMs", cfg?.clickMs ?? 30, 5, 200)
    + numberInput("미리 누르기 (ms)", "터치가 늦게 먹히면 이만큼 미리 누름 · 기본 0 · 릴링이 자꾸 실패하면 40 ~ 80", "leadMs", cfg?.leadMs ?? 0, 0, 300)
    + numberInput("목표 위치 (%)", "구간 왼쪽 끝에서 구간 폭의 몇 % · 기본 0", "targetPercent", cfg?.targetPercent ?? 0, 0, 60)
    + toggle("터치 지연에 맞춰 기다리기", 'data-tuning="adaptive"', cfg?.adaptive ?? true)
    + text("누른 뒤 반응이 보일 때까지 다시 누르지 않는 시간을 이 폰의 터치 속도에 맞춤", 12)
    + toggle("릴링 때 미니게임 창 위쪽을 누름", 'data-tuning="tapAbove"', cfg?.tapAbove ?? true)
    + text("모바일은 미니게임 창 바로 아래가 Roll 버튼이라 위쪽 빈 화면 (캐릭터 쪽) 을 누름 · 끄면 창 아래를 누름", 12)) + move();
}

function move() {
  const fish = group("mfish");
  return card("movement", "이동 · 판매",
    text("위에서 내려다보며 현재 위치와 장애물을 확인해 이동합니다. 바닷가는 낚시 버튼이 보이는 곳까지 찾습니다.", 12)
    + row("캐릭터 이름 (표시명|계정명)", "비우면 바이옴 설정의 플레이어 이름을 사용합니다.", `<input data-macro="player_label" aria-label="${tr("캐릭터 이름 (표시명|계정명)")}" maxlength="160" value="${escape(String(macro.player_label ?? ""))}">`)
    + toggle("시작할 때 낚시 장소로 이동", 'data-macro="useMove"', macro.useMove !== false && fish.useMove !== false)
    + toggle("가득 차면 판매하러 가기", 'data-macro="useSell"', macro.useSell !== false && fish.useSell !== false)
    + [ ["base", "기준 장소"], ["coast", "낚시 장소"], ["sell", "판매 장소"], ["craft", "포션 제작 장소"], ["match", "메모리 매치 장소"] ].map(([destination, name]) => row(name, "목적지 앞에서 현재 장소 기억 · 다음 이동에 실제 화면 사용", `<div class="buttons">${button("이동", "go:" + destination)}${button("현재 장소 기억", "mark:" + destination)}</div>`)).join("")
    + `<div class="buttons">${button("관측 취소", "task:waypoint_cancel")}${button("이동 정지", "task:move_stop", "danger")}</div>`)
  + card("sell", "판매", macroNumber("mfish", "e_wait", "대화 후 대기 (초)", .1, 30, .1)
    + macroNumber("mfish", "sell_max", "판매 최대 종류", 1, 500, 1)
    + macroNumber("mfish", "sell_delay", "판매 클릭 추가 대기 (초)", 0, 5, .1)
    + `<div class="buttons">${button("판매 위치 자동 보정", "task:sell_autocal")}${button("판매 테스트", "task:sell_test")}</div>`)
  + card("places", "이동 조작 위치", positionRows("base", [["stick_center", "조이스틱 가운데"], ["stick_edge", "조이스틱 바깥쪽"], ["jump_btn", "점프 버튼"]])
    + text("지형과 목적지는 실제 화면에서 확인하며, 벽에 닿았다는 이유만으로 점프하지 않습니다.", 12)
    + `<div class="buttons">${button("제작 장소", "go:craft")}${button("메모리 매치 장소", "go:match")}${button("위치 · 길 모두 지우기", "clear-navigation", "danger")}</div>`);
}
function logs() {
  return card("recent", "기록", text("최근 기록 (최대 300줄)", 12)
    + `<pre id="records">${tr("아직 기록 없음")}</pre><div class="buttons">${button("기록 지우기", "reset:log")}</div>`);
}

function macroNumber(feat: string, key: string, label: string, min = 0, max = 600, step = .1) {
  return row(label, "", `<input type="number" data-macro="${feat}.${key}" aria-label="${tr(label)}" value="${Number(group(feat)[key] ?? 0)}" min="${min}" max="${max}" step="${step}">`);
}
function macroText(feat: string, key: string, label: string) {
  return row(label, "", `<input type="text" data-macro="${feat}.${key}" aria-label="${tr(label)}" value="${escape(String(group(feat)[key] ?? ""))}" maxlength="500">`);
}
function positionRows(feat: string, fields: readonly (readonly [string, string])[]) {
  return fields.map(([key, label]) => row(label, Array.isArray(group(feat)[key]) ? "✓ " + (group(feat)[key] as number[]).map(v => v.toFixed(3)).join(", ") : "미지정 · 화면에서 글자로 먼저 찾음",
    button(key.endsWith("_region") ? "영역 지정" : "위치 지정", `position:${feat}.${key}`))).join("");
}
const merchantItems: Record<string, string[]> = {
  Mari: ["Void Coin", "Lucky Penny", "Gear A", "Gear B", "Mixed Potion", "Speed Potion", "Lucky Potion", "Fortune Spoid I"],
  Jester: ["Oblivion Potion", "Heavenly Potion", "Potion of Bound", "Rune of Everything", "Random Potion Sack", "Stella's Candle", "Lucky Potion"],
};
const potionBiomes = ["CYBERSPACE", "GLITCHED", "DREAMSPACE"];
function potionTemplate(biome: string) {
  const pop = group("mpop");
  const template = (pop.templates as Bag)[biome] as Bag | undefined;
  const enabled = (pop.biomes_on as Bag)[biome] === true;
  return `<details><summary>${biome}</summary>`
    + toggle("이 바이옴에서 사용", `data-macro="mpop.biomes_on.${biome}"`, enabled)
    + `<div class="buttons">${button("기본 템플릿", "template:" + biome)}${button("테스트", "pop-test:" + biome)}</div>`
    + (template ? `<label class="row"><span>${tr("사용 방식")}</span><select data-template="${biome}.mode"><option value="one" ${template.mode === "one" ? "selected" : ""}>${tr("순서대로 하나씩")}</option><option value="all" ${template.mode === "all" ? "selected" : ""}>${tr("전부 사용")}</option></select></label>`
      + ((template.items as Bag[]) ?? []).map((item, index) => `<div class="card">`
        + row("포션 이름", "", `<input data-template="${biome}.${index}.name" value="${escape(String(item.name ?? ""))}" aria-label="${tr("포션 이름")}">`)
        + row("사용 수량", "ALL = 전부", `<input data-template="${biome}.${index}.amount" value="${escape(String(item.amount ?? "ALL"))}" aria-label="${tr("사용 수량")}">`)
        + row("최소 보유 수량", "", `<input type="number" min="0" max="1000000000" step="1" data-template="${biome}.${index}.min_have" value="${Number(item.min_have ?? 1)}" aria-label="${tr("최소 보유 수량")}">`)
        + button("삭제", `template-delete:${biome}.${index}`, "danger") + `</div>`).join("")
      + button("포션 추가", "template-add:" + biome) : text("기본 템플릿을 불러온 뒤 사용할 포션과 수량을 바꿀 수 있습니다.", 12))
    + "</details>";
}
function macroSettings() {
  const enabled = (feat: string) => toggle("사용", `data-macro="${feat}.enabled"`, group(feat).enabled === true);
  const close = (feat: string) => toggle("사용 후 인벤토리 닫기", `data-macro="${feat}.close_inventory"`, group(feat).close_inventory !== false);
  const tasks = (feat: string, extra = "") => `<div class="buttons">${button("테스트", "task:" + feat + "_test")}${extra}${button("중지", "task:" + feat + "_stop", "danger")}</div>`;
  return card("inventory", "공통 위치 · 인벤토리",
    text("Android 13 이상은 기존 키보드를 유지해 검색어와 수량을 입력합니다. 구형 Android는 홈에서 Acrux 입력기를 준비하세요. 위치가 비어 있으면 화면 글자를 먼저 찾습니다.", 12)
    + positionRows("base", [["inventory_pos", "Inventory 버튼"], ["items_pos", "Items 버튼"], ["search_pos", "Search 입력칸"], ["item_pos", "검색 결과 첫 칸"], ["amount_pos", "수량 입력칸"], ["use_pos", "Use 버튼"], ["ocr_region", "아이템 이름 · 수량 영역"], ["notice_region", "알림 영역"], ["chat_pos", "채팅 버튼"], ["dialog_pos", "대화창"]])
    + macroNumber("pop", "match_threshold", "아이템 이름 일치율 (%)", 0, 100, 1)
    + `<details><summary>${tr("인벤토리 대기 시간")}</summary>${["inventory", "items", "search", "typing", "item_click", "dbl_gap", "after_dbl", "after_enter"].map(key => row(key, "초", `<input type="number" min="0" max="60" step="0.05" data-macro="pop.delays.${key}" value="${Number((group("pop").delays as Bag)[key] ?? .5)}">`)).join("")}</details>`
    + `<div class="buttons">${button("아이템 OCR 확인", "task:mpop_ocr_test")}${button("OCR 정보", "task:ocr_info")}</div>`)
  + card("popping", "오토 팝핑", enabled("mpop")
    + macroNumber("mpop", "start_delay", "바이옴 감지 후 대기 (초)", 0, 60, .1) + close("mpop")
    + potionBiomes.map(potionTemplate).join("")
    + `<div class="buttons">${button("중지", "task:mpop_stop", "danger")}</div>`)
  + card("items", "오토 아이템", enabled("mitem")
    + toggle("Strange Controller", 'data-macro="mitem.strange"', group("mitem").strange !== false)
    + macroNumber("mitem", "strange_min", "Strange Controller 간격 (분)", 1, 1440, .1)
    + toggle("Biome Randomizer", 'data-macro="mitem.randomizer"', group("mitem").randomizer !== false)
    + macroNumber("mitem", "randomizer_min", "Biome Randomizer 간격 (분)", 1, 1440, .1)
    + close("mitem") + tasks("mitem"))
  + card("blood", "블러드 레인 자동 생성", enabled("mblood")
    + text("PUMPKIN MOON · GRAVEYARD 감지 시 Cursed Rune Fragment를 한 번 사용합니다.", 12)
    + macroNumber("mblood", "start_delay", "바이옴 감지 후 대기 (초)", 0, 60, .1) + close("mblood") + tasks("mblood"))
  + card("merchant", "상인 자동 구매", enabled("mmerch")
    + macroNumber("mmerch", "check_sec", "채팅 확인 간격 (초)", 10, 600, 1)
    + macroNumber("mmerch", "teleport_wait", "순간이동 후 대기 (초)", .5, 60, .1)
    + toggle("상점 자동 보정", 'data-macro="mmerch.auto_cal"', group("mmerch").auto_cal !== false)
    + Object.entries(merchantItems).map(([who, items]) => `<details><summary>${who}</summary>`
      + items.map(item => toggle(item, `data-buy="${escape(who + "_" + item)}"`, Number((group("mmerch").buy as Bag)[who + "_" + item]) > 0)).join("")
      + button("구매 테스트", "merchant:" + who) + "</details>").join("")
    + positionRows("mmerch", [["chat_region", "채팅 글자 영역"], ["open_pos", "Open 선택지"], ["first_slot", "첫 번째 칸"], ["second_slot", "두 번째 칸"], ["item_region", "아이템 이름 영역"], ["max_pos", "Set to Max 버튼"], ["purchase_pos", "Purchase 버튼"], ["close_pos", "상점 닫기 X"]])
    + `<div class="buttons">${button("채팅 확인", "task:mmerch_check")}${button("자동 보정", "task:mmerch_autocal")}${button("중지", "task:mmerch_stop", "danger")}</div>`)
  + card("crafting", "자동 포션 제작", enabled("mcraft") + macroText("mcraft", "potion", "제작할 포션 이름")
    + macroNumber("mcraft", "f_wait", "제작 창 열기 후 대기 (초)", .1, 60, .1)
    + positionRows("mcraft", [["search_pos", "Search 입력칸"], ["list_region", "검색 결과 목록"], ["shop_close_pos", "제작 창 X"], ["open_recipe_pos", "Open Recipe 버튼"], ["add_all_pos", "Add Everything 버튼"], ["craft_pos", "Craft 버튼"], ["add_close_pos", "재료 창 X"]])
    + tasks("mcraft", button("자동 보정", "task:mcraft_autocal")))
  + card("memory", "자동 메모리 매치", enabled("mmatch")
    + macroNumber("mmatch", "e_wait", "보드 열기 후 대기 (초)", .5, 60, .1)
    + macroNumber("mmatch", "flip_wait", "카드 뒤집힌 뒤 대기 (초)", .1, 5, .05)
    + positionRows("mmatch", [["note_region", "알림 창 영역"], ["note_close_pos", "알림 창 X"], ["note_btn_pos", "Start / Available 버튼"], ["ad_pos", "Watch AD 버튼"], ["board_close_pos", "카드 판 Close 버튼"]])
    + tasks("mmatch", button("알림 창 자동 보정", "task:mmatch_autocal") + button("카드 판 자동 보정", "task:mmatch_boardcal")))
  + card("fish-positions", "낚시 · 판매 위치",
    positionRows("mfish", [["panel_region", "낚시 대기 창 영역"], ["reel_region", "미니게임 창 영역"], ["bar_region", "릴링 바 영역"], ["result_region", "결과창 영역"], ["fish_btn", "Fish / Exit 버튼"], ["close_pos", "결과창 X"], ["title_pos", "결과창 제목"], ["sell_fish_pos", "Sell Fish 버튼"], ["first_fish_pos", "첫 번째 물고기 칸"], ["sell_all_pos", "Sell All 버튼"], ["confirm_sell_pos", "확인 Sell 버튼"], ["shop_close_pos", "상점 닫기 X"]]))
  + card("recovery", "게임 입장 · 복귀", macroText("ret", "ps_link", "비공개 서버 링크")
    + macroNumber("ret", "start_wait", "입장 후 대기 (초)", 0, 600, .5)
    + macroNumber("play", "load_wait", "Play 클릭 전 대기 (초)", 0, 120, .5)
    + macroNumber("play", "interval", "Play / Skip 클릭 간격 (초)", .05, 10, .05)
    + macroNumber("play", "max_time", "입장 최대 대기 (초)", 5, 1800, 1)
    + positionRows("play", [["pos", "Play 버튼"], ["skip_pos", "Click to skip 버튼"]])
    + `<div class="buttons">${button("서버 접속", "task:ret_join")}${button("복귀 테스트", "task:ret_test")}${button("Play 테스트", "task:play_test")}${button("Play 중지", "task:play_stop", "danger")}${button("자동 복귀 취소", "task:crash_cancel")}</div>`);
}

function settings() {
  const version = current?.version || "0.1.0";
  return card("update", "업데이트",
    row("Acrux Mobile V" + version, `지금 V${version} · 새 버전이 나오면 받아서 바로 설치 (설정은 그대로)`, button("확인", "update:check", "accent"))
    + toggle("앱을 켤 때 자동으로 업데이트", 'data-macro="auto_update"', macro.auto_update === true)
    + '<p id="update-state" class="readout"></p>'
    + `<div class="buttons">${button("업데이트 받기", "update:download", "accent", "update-download")}${button("설치", "update:install", "accent", "update-install")}</div>`)
  + macroSettings()
  + card("discord-link", "디스코드 연결",
    text("연결하면 디스코드 봇 (/상태 · /통계 · /로그) 으로 이 매크로를 볼 수 있음", 12)
    + row("디스코드 계정", "", '<strong id="discord-name">-</strong>')
    + `<p id="discord-state" class="readout"></p><div class="buttons">${button("디스코드 로그인", "discord-link", "accent", "discord-action")}</div>`
    + text("연결한 동안 보내는 것: 매크로 작동 여부 · 지금 하는 일 · 켜진 기능 · 바이옴 · 통계 · 최근 기록 30줄 (작동 중 15초 · 꺼져 있으면 1분마다)", 12)
    + text("화면 · 파일 · 웹후크 주소는 보내지 않음 · 연결된 앱은 acruxmacro.com/link 에서도 끊을 수 있음", 12))
  + card("overlay-settings", "조작 창",
    text("떠 있는 Acrux 창 · 어디든 잡고 끌면 움직임", 12)
    + `<div class="buttons">${button(overlay.visible ? "UI 끄기" : "UI 켜기", "toggle-ui", overlay.visible ? "muted" : "accent")}</div>`
    + text("매크로를 안 돌릴 때 떠 있는 창을 통째로 끔 · 앱 맨 위 버튼과 같음 · 끄면 돌던 매크로도 멈춤", 12)
    + toggle("Acrux 앱을 볼 때는 숨기기", 'id="hide-on-app"', overlay.hideOnApp)
    + `<label class="opacity">${tr("조작 창 투명도")}<input id="opacity" type="range" min="40" max="100" value="${overlay.alpha}"></label>`
    + `<div class="buttons">${button("조작 창 위치 되돌리기", "reset-overlay")}</div>`)
  + card("info", "정보",
    text("사용자 수 집계: 켜져 있다는 신호만 보냄 (무작위 번호 · 개인 정보 없음)", 12)
    + toggle("사용자 수 집계 참여", 'id="online-share"', online.share)
    + text("Acrux Mobile V" + version, 13, "fg") + text("Made by 얼로니", 12));
}

function biomePage() {
  const cfg = biome.settings;
  return card("wireless", "무선 디버깅 연결",
    text("로블록스 로그로 바이옴을 읽으려면 로그를 읽을 권한이 필요해요. 안드로이드의 무선 디버깅 기능으로 Acrux 가 이 폰에 직접 연결해요 [ 안드로이드 11 이상, 와이파이 필요 ]")
    + '<p id="adb-state" class="readout"></p>'
    + ["1. 설정 → 휴대전화 정보 → 소프트웨어 정보 → '빌드 번호' 7번 눌러서 개발자 옵션 켜기",
      "2. 아래 [페어링 시작] → 개발자 옵션 → 무선 디버깅 켜기 → '페어링 코드로 기기 페어링'",
      "3. 코드 창을 띄운 채로 화면 위를 내려서 Acrux 알림의 [코드 입력] 에 6자리 코드",
      "4. 한 번 페어링하면 다음부터는 무선 디버깅만 켜져 있으면 [연결] (재부팅 · 와이파이가 바뀌면 다시 켜기)"].map(step => text(step, 13, "fg")).join("")
    + `<div class="buttons">${button("페어링 시작", "biome:pair", "accent")}${button("연결", "biome:connect")}${button("개발자 옵션", "biome:developer")}</div>`)
  + card("biome-macro", "바이옴 매크로", '<p id="biome-state" class="readout"></p>'
    + toggle("바이옴 매크로 켜기", 'data-biome="enabled"', cfg.enabled)
    + row("플레이어 이름", "게임 로그에 표시되는 계정 이름", `<input data-biome="player" aria-label="${tr("플레이어 이름")}" value="${escape(cfg.player)}">`)
    + text("디스코드 웹후크 (여러 개면 줄바꿈)", 13, "fg")
    + `<textarea data-biome="webhooks" rows="2" spellcheck="false" aria-label="${tr("디스코드 웹후크")}" placeholder="https://discord.com/api/webhooks/…">${escape(cfg.webhooks.join("\n"))}</textarea>`
    + text("비공개 서버 링크", 13, "fg")
    + `<input class="wide" data-biome="psLink" type="url" aria-label="${tr("비공개 서버 링크")}" placeholder="https://www.roblox.com/share?code=…" value="${escape(cfg.psLink)}">`
    + toggle("레어 바이옴 @everyone [ GLITCHED, DREAMSPACE, CYBERSPACE ]", 'data-biome="everyone"', cfg.everyone)
    + `<div class="buttons">${button("웹후크 테스트", "webhook-test")}</div>`)
  + card("biome-log-test", "바이옴 로그 테스트", text("모바일 로블록스가 바이옴 정보를 로그에 남기는지 확인해요.")
    + text("무선 디버깅을 연결한 뒤 [테스트 시작] → 로블록스로 Sol's RNG 에 들어가서 90초 동안 두기 (바이옴이 바뀌는 때면 더 좋아요). 결과는 기록 칸에 떠요.", 13, "fg")
    + `<div class="buttons">${button("테스트 시작 (90초)", "biome:testLogs", "accent", "biome-test-button")}</div>`
    + text("결과 (기록 칸에도 같이 떠요 · 끝나면 알림)", 12)
    + '<pre id="biome-test-output"></pre>');
}

const renderers = { home, fish: fishing, biome: biomePage, log: logs, settings };
function renderPage(restoreScroll = true) {
  const main = element("pages");
  main.innerHTML = '<nav id="jumps"></nav><div id="scroll"><div class="cards" data-page="' + page + '">' + renderers[page]() + "</div></div>";
  const jump = element("jumps");
  const sections = [...main.querySelectorAll<HTMLElement>(".card")];
  jump.hidden = sections.length < 3;
  if (!jump.hidden) {
    jump.innerHTML = sections.map(card => `<button class="chip" data-action="jump:${card.id}">${card.dataset.title}</button>`).join("");
  }
  const scroll = element("scroll");
  if (restoreScroll) scroll.scrollTop = pageMemory.get(page) ?? 0;
  const mark = () => {
    if (jump.hidden) return;
    const at = scroll.scrollTop > 0 && scroll.scrollHeight - scroll.scrollTop <= scroll.clientHeight + 1
      ? sections.length - 1 : sections.findLastIndex(section => section.offsetTop <= scroll.scrollTop + 40);
    [...jump.children].forEach((child, index) => child.setAttribute("aria-pressed", String(index === Math.max(0, at))));
  };
  scroll.addEventListener("scroll", mark, { passive: true });
  mark();
  applyStatus();
}

function render() {
  document.documentElement.lang = overlay.lang;
  const version = current?.version ?? "0.1.0";
  element("header").innerHTML = `<div><h1>Acrux Mobile <span>V${version}</span></h1><p>${tr("Sol's RNG 매크로")}</p><div class="online"><span>●</span>Now Active : <span id="online-count">--</span></div></div>`
    + button(overlay.visible ? "UI 끄기" : "UI 켜기", "toggle-ui", overlay.visible ? "muted" : "accent");
  element("tabs").innerHTML = tabs.map(([id, name]) =>
    `<button class="tab" data-action="tab:${id}" aria-selected="${id === page}">${tr(name)}</button>`).join("");
  renderPage();
  renderGuide();
}

function select(next: Page) {
  if (next === page) return;
  pageMemory.set(page, element("scroll").scrollTop);
  page = next;
  localStorage.setItem(tabKey, JSON.stringify(page));
  render();
  if (page === "settings") void refreshDiscord();
}

function setText(id: string, value: string) {
  const target = document.getElementById(id);
  if (target && target.textContent !== value) target.textContent = value;
}

function macroActive() {
  return macroPending === "start" || macroRestartRevision !== null || !!current?.dua.running || !!current && current.phase !== "idle";
}

function applyDua() {
  const d = current?.dua;
  const active = macroActive();
  setText("macro-toggle", t(macroPending === "stop" ? "정지 중…" : active ? "정지" : "시작"));
  const button = document.getElementById("macro-toggle") as HTMLButtonElement | null;
  if (button) {
    button.disabled = macroPending === "stop" || !active && (!d?.supported || !current?.captureReady || !current?.inputReady);
    button.classList.toggle("danger", active);
    button.classList.toggle("accent", !active);
  }
  setText("dua-state", t(macroPending === "start" ? "시작 중…" : macroPending === "stop" ? "정지 중…" : d?.running ? "실행 중" : "중지됨")
    + " · " + t(d?.route === "headless" ? "Headless" : d?.route === "screenshot" ? "스크린샷" : "대기"));
  setText("dua-reason", t(d?.reason ?? "DUA 대기 중"));
  setText("dua-counts", t("Headless {0} · 스크린샷 {1} · 경로 전환 {2}")
    .replace("{0}", String(d?.headlessTicks ?? 0)).replace("{1}", String(d?.screenshotFrames ?? 0)).replace("{2}", String(d?.transitions ?? 0)));
  setText("dua-guard", current?.platform === "android"
    ? t("Roblox 화면에서만 터치 · 다른 앱에서는 대기") + "\n" + t(current.gameAvailable ? "✓ Roblox 입력 가능" : "Roblox가 앞에 오기를 기다립니다.") : "");
}

async function changeMacro() {
  if (macroPending === "stop") return;
  const stopping = macroActive();
  if (!stopping && (!current?.dua.supported || !current.captureReady || !current.inputReady)) {
    message("화면 캡처와 입력을 먼저 준비하세요.");
    return;
  }
  const revision = ++macroRevision;
  if (stopping) macroRestartRevision = null;
  ++request;
  macroPending = stopping ? "stop" : "start";
  applyDua();
  try {
    let next: Status;
    if (stopping) next = await duaStop();
    else {
      await macroSaves;
      await numberSaves;
      if (!macroLoaded) await loadMacro();
      if (revision !== macroRevision) return;
      const selected = selectedFeatures();
      if (!selected.length) throw new Error("홈에서 실행할 기능을 하나 이상 켜세요.");
      const fresh = await status();
      if (revision !== macroRevision) return;
      next = await invoke<Status>("dua_start", { features: selected, generation: fresh.generation });
    }
    if (revision === macroRevision) { current = next; ++request; }
  } catch (error) {
    if (revision === macroRevision) message(String(error));
  } finally {
    if (revision === macroRevision) {
      macroPending = null;
      applyStatus();
      void refresh();
    }
  }
}

function applyDiscord() {
  setText("discord-name", discord.linked ? discord.name || "-" : "-");
  const code = discord.code && discord.code.length === 8 ? discord.code.slice(0, 4) + "-" + discord.code.slice(4) : discord.code;
  const hint = discord.linked ? t("끊으면 봇에서 이 매크로가 사라짐")
    : code ? t("브라우저에서 [연결] 을 눌러 주세요 · 코드 {0}").replace("{0}", code)
    : t("브라우저가 열리면 디스코드로 로그인한 뒤 [연결] 을 누르면 끝");
  setText("discord-state", discord.error ? hint + "\n" + t(discord.error) : hint);
  setText("discord-action", t(discord.linked ? "연결 끊기" : discord.code ? "취소" : "디스코드 로그인"));
  const button = document.getElementById("discord-action") as HTMLButtonElement | null;
  if (button) button.disabled = discordBusy || !discordLoaded;
}

function scheduleDiscord() {
  if (discordTimer !== undefined) clearTimeout(discordTimer);
  discordTimer = undefined;
  if (!document.hidden && page === "settings" && (discord.code || discord.linked)) {
    discordTimer = setTimeout(() => { void refreshDiscord(); }, discord.code ? 2000 : 10000);
  }
}

async function refreshDiscord() {
  if (discordBusy || document.hidden || page !== "settings") return;
  const revision = ++discordRevision;
  try {
    const next = await invoke<DiscordState>("discord_state");
    if (revision === discordRevision) { discord = next; discordLoaded = true; }
  } catch (error) {
    if (revision === discordRevision) discord = { ...discord, error: String(error) };
  }
  if (revision !== discordRevision) return;
  applyDiscord();
  scheduleDiscord();
}

async function changeDiscord() {
  if (discordBusy || !discordLoaded) return;
  discordBusy = true;
  const revision = ++discordRevision;
  if (discordTimer !== undefined) clearTimeout(discordTimer);
  applyDiscord();
  try {
    const command = discord.linked ? "discord_logout" : discord.code ? "discord_cancel" : "discord_login";
    const next = await invoke<DiscordState>(command);
    if (revision === discordRevision) {
      discord = next;
      if (discord.error) message(discord.error);
    }
  } catch (error) {
    if (revision === discordRevision) {
      discord = { ...discord, error: String(error) };
      message("디스코드 연결 실패");
    }
  } finally {
    discordBusy = false;
    applyDiscord();
    scheduleDiscord();
  }
}

function statsText(prefix: string, s: Stats) {
  const total = s.success + s.junk + s.fail;
  [s.success, s.junk, s.fail, total ? ((s.success + s.junk) / total * 100).toFixed(1) + "%" : "-"]
    .forEach((value, i) => setText(`${prefix}-${i}`, String(value)));
}

function applyStatus() {
  const s = current;
  const session = s?.stats ?? zero;
  setText("access-state", t(s?.inputReady ? "✓ 켜짐" : "꺼짐"));
  setText("access-button", t(s?.inputReady ? "설정" : "켜기"));
  setText("capture-state", t(s?.captureReady ? "✓ 캡처 중" : "꺼짐"));
  setText("capture-button", t(s?.captureReady ? "중지" : "시작"));
  setText("input-state", t(inputMethod.selected ? "✓ 선택됨" : inputMethod.enabled ? "입력기 선택 필요" : "꺼짐"));
  setText("input-button", t(inputMethod.enabled && !inputMethod.selected ? "선택" : "설정"));
  const auto = (s as (Status & { automation?: Bag }) | undefined)?.automation;
  setText("automation-state", auto ? String(auto.message ?? "") + ` · ${t("완료")} ${auto.completed ?? 0} · ${t("실패")} ${auto.failed ?? 0}` : "");
  setText("update-state", String(update.error ?? update.message ?? ""));
  const download = document.getElementById("update-download") as HTMLButtonElement | null;
  const install = document.getElementById("update-install") as HTMLButtonElement | null;
  if (download) download.disabled = !update.available || !!update.busy;
  if (install) install.disabled = !update.ready || !!update.busy;
  document.getElementById("access-state")?.classList.toggle("ok", !!s?.inputReady);
  document.getElementById("capture-state")?.classList.toggle("ok", !!s?.captureReady);
  setText("run-state", t(s?.message === "대기 중" || !s ? "대기" : s.message) + "\n"
    + t("성공 {0} · 쓰레기 {1} · 실패 {2}").replace("{0}", String(session.success)).replace("{1}", String(session.junk)).replace("{2}", String(session.fail))
    + "  (" + t("이번 실행") + ")");
  setText("cal-state", t(s?.calibrated ? "낚시 보정 저장됨" : "시작하면 자동으로 보정한 뒤 낚시합니다."));
  applyDua();
  statsText("run", session);
  statsText("total", s?.totals ?? zero);
  setText("online-count", online.online === null ? "--" : String(online.online));
  const share = document.getElementById("online-share") as HTMLInputElement | null;
  if (share) share.checked = online.share;
  setText("adb-state", t(transport.connection === "ok" ? "✓ 무선 디버깅 연결됨" : transport.connection === "pair" ? "무선 디버깅 페어링 필요" : "무선 디버깅 연결 안 됨"));
  setText("biome-state", t("지금 바이옴: ") + (biome.current ?? "-") + " · " + t(biome.message));
  setText("biome-test-output", transport.testOutput || t("아직 기록 없음"));
  setText("biome-test-button", t(transport.testUntil > Date.now() ? "테스트 중…" : "테스트 시작 (90초)"));
  const testButton = document.getElementById("biome-test-button") as HTMLButtonElement | null;
  if (testButton) testButton.disabled = transport.testUntil > Date.now();
  applyDiscord();
  for (const field of document.querySelectorAll<HTMLInputElement>("input[data-biome]")) {
    if (field.type === "checkbox") field.checked = !!biome.settings[field.dataset.biome as "enabled" | "everyone"];
  }
  if (page === "log") void invoke<string[]>("fishing_log").then(lines =>
    setText("records", lines.length ? lines.map(t).join("\n") : t("아직 기록 없음"))).catch(() => {});
}

async function setup(action: Setup, options?: (Overlay & { reset?: boolean }) | { action: string }) {
  return invoke<void>("android_setup", { action, options });
}

async function syncOverlay(reset = false) {
  if (current?.platform === "android") await setup("overlay", { ...overlay, reset });
  localStorage.setItem(prefsKey, JSON.stringify(overlay));
}

async function autoUpdateOnLaunch() {
  if (macro.auto_update !== true) return;
  try {
    update = await serviceAction("update_check");
    if (update.available) {
      update = await serviceAction("update_download");
      if (update.ready) update = { ...update, ...await serviceAction("update_install") };
    }
    applyStatus();
  } catch (error) { update = { ...update, error: String(error) }; applyStatus(); }
}

async function refresh() {
  if (document.hidden || busy || macroSaving) return;
  const ticket = ++request;
  try {
    const next = await status();
    if (!macroLoaded) await loadMacro();
    inputMethod = await invoke<typeof inputMethod>("android_input_status");
    if (page === "settings") update = await serviceAction("update_state");
    biome = await biomeStatus();
    transport = await invoke<typeof transport>("biome_transport");
    online = await invoke<typeof online>("online_status");
    if (ticket !== request) return;
    const initial = !current;
    current = next;
    if (initial) {
      render();
      void autoUpdateOnLaunch();
      if (page === "settings") void refreshDiscord();
      if (next.platform === "android") {
        void setup("biome", { action: "refresh" }).catch(error => message(String(error)));
        void syncOverlay().catch(error => message(String(error)));
        if (!guide && !restore("acrux.mobile.ready-guide", false) && (!next.inputReady || !next.captureReady)) startGuide("ready");
      }
    } else {
      applyStatus();
      guideStatus();
    }
  } catch (error) {
    if (ticket === request) message(String(error));
  }
}

interface Step {
  title: string;
  body: string;
  check?: "inputReady" | "captureReady" | "calibrated" | "textInputReady";
  action?: "accessibility" | "capture" | "inputSetup";
  optional?: boolean;
}

const guideSteps: Record<GuideId, Step[]> = {
  ready: [
    { title: "Acrux Mobile 에 오신 걸 환영합니다.", body: "이 매크로는 Sol's RNG 전용 매크로이며, 다양한 모바일 기능을 지원합니다.\n\n시작하기 전에, 매크로가 작동할 때 필요한 필수 권한 두 개를 켜야 합니다." },
    { title: "접근성 권한 켜기", body: "매크로가 로블록스 화면을 터치하는 데 필요한 권한입니다.\n\n설정 → 접근성 → 설치된 앱 → Acrux Mobile → 켜기\n\n'제한된 설정' 으로 켜지지 않는다면, 설정 → 앱 → Acrux Mobile → 오른쪽 위 ⋮ → '제한된 설정 허용' 후 다시 켜 주세요.", check: "inputReady", action: "accessibility" },
    { title: "화면 캡처 시작", body: "매크로가 화면을 보고 낚시 상태와 버튼을 찾는 데 필요합니다. 화면은 녹화되거나 저장되지 않습니다.\n\n공유 창이 뜨면 '전체 화면' 을 선택해 주세요.", check: "captureReady", action: "capture" },
    { title: "글자 입력 준비", body: "Android 13 이상은 기존 키보드를 그대로 사용합니다. 구형 Android에서 아이템 검색과 수량 입력을 사용할 때만 Acrux 입력기를 켜고 선택하세요. 낚시만 사용하면 이 단계를 건너뛰어도 됩니다.", check: "textInputReady", action: "inputSetup", optional: true },
    { title: "로블록스 준비", body: "홈에서 실행할 기능을 켜고 로블록스 (Sol's RNG)를 가로 전체 화면으로 여세요. 이동을 켰다면 실제 화면에서 해안과 목적지를 찾고, 위치나 지형이 모호하면 멈춥니다.\n\n이동을 끈 낚시는 Fish 버튼이 보이는 바닷가에서 시작하세요. 다른 앱에서는 입력을 기다리고 화면이 잠기면 정지합니다." },
    { title: "준비 완료", body: "선택한 기능의 설정과 위치를 확인한 뒤 로블록스를 열어 시작하세요. 낚시가 선택돼 있으면 다음 보정 안내가 이어집니다." },
  ],
  fish: [
    { title: "낚시 보정", body: "[시작] 을 누르면 저장된 보정이 없을 때 자동으로 보정한 뒤 낚시합니다. 수동 보정은 Acrux 창 ⚙ → [낚시] 에서 할 수 있습니다." },
    { title: "낚시 자리로 이동", body: "로블록스에서 낚시 자리로 이동해 Fish 버튼이 보이게 해 주세요.\n\nAcrux 창이 낚시 창을 가린다면 위쪽 구석으로 옮겨 주세요." },
    { title: "낚시 자동 보정", body: "로블록스 위 Acrux 창의 [시작] 을 누르세요. 저장된 보정이 없으면 실제 버튼과 낚시 패널을 읽어 보정한 뒤 낚시를 시작합니다.\n\n진행 상태와 정지 버튼은 Acrux 창에서 확인하세요.", check: "calibrated" },
    { title: "낚시 보정 완료", body: "자동 낚시는 [정지] 로 멈출 수 있습니다. 판매가 켜져 있으면 인벤토리가 가득 찼을 때 판매하러 이동합니다." },
  ],
  move: [
    { title: "이동 설정", body: "낚시 칸에서 이동과 판매를 켜고 조이스틱과 점프 위치를 지정하세요. 화면에서 현재 위치와 장애물을 확인해 목적지로 이동합니다." },
  ],
  sell: [
    { title: "판매 설정", body: "판매 위치를 자동 보정하거나 판매 버튼 위치를 지정하세요. 판매 테스트는 판매 장소로 이동해 판매한 뒤 낚시를 재개합니다." },
  ],
};
let guide: { id: GuideId; index: number; wasReady: boolean } | null = null;

function ready(step: Step) {
  return !step.check || (step.check === "textInputReady" ? inputMethod.selected : !!current?.[step.check]);
}

function startGuide(id: GuideId) {
  guide = { id, index: 0, wasReady: true };
  renderGuide();
}

function closeGuide() {
  if (guide?.id === "ready") localStorage.setItem("acrux.mobile.ready-guide", "true");
  guide = null;
  renderGuide();
}

function renderGuide() {
  const dialog = element<HTMLDialogElement>("guide");
  if (!guide) {
    if (dialog.open) dialog.close();
    return;
  }
  const steps = guideSteps[guide.id];
  const step = steps[guide.index];
  const badge = guide.id === "ready" && !current?.calibrated ? "남은 튜토리얼 1개" : "튜토리얼";
  dialog.innerHTML = `<b class="guide-badge">${tr(badge)}</b><div class="guide-label"><b>${tr(guideNames[guide.id])}</b><span>${guide.index + 1} / ${steps.length}</span></div>
    <h2 id="guide-title">${tr(step.title)}</h2><p class="guide-body readout">${tr(step.body)}</p>
    ${step.check === "calibrated" ? text("로블록스 화면의 Acrux 창에서 해 주세요", 13, "warning") : ""}
    ${step.action ? `<div class="buttons guide-action">${button(step.action === "capture" ? "시작" : "설정", step.action, "accent")}<strong id="guide-action-state"></strong></div>` : ""}
    <div class="guide-controls">
      ${guide.index === 0 ? button("나중에", "guide-close") : button("이전", "guide-prev")}
      ${step.optional ? button("이 단계 건너뛰기", "guide-skip") : ""}
      ${button(guide.index === steps.length - 1 ? "완료" : "다음", "guide-next", "accent", "guide-next")}
    </div>`;
  element<HTMLButtonElement>("guide-next").disabled = !ready(step);
  showGuideAction(step);
  if (!dialog.open) dialog.showModal();
}

function moveGuide(direction: number, skip = false) {
  if (!guide) return;
  const steps = guideSteps[guide.id];
  const step = steps[guide.index];
  if (direction > 0 && !ready(step) && !(step.optional && skip)) return;
  const next = guide.index + direction;
  if (next < 0) return;
  if (next === steps.length) {
    const id = guide.id;
    closeGuide();
    if (id === "ready" && selectedFeatures().includes("mfish") && !current?.calibrated) startGuide("fish");
    return;
  }
  guide.index = next;
  guide.wasReady = ready(steps[next]);
  renderGuide();
}

function guideStatus() {
  if (!guide) return;
  const step = guideSteps[guide.id][guide.index];
  const ok = ready(step);
  element<HTMLButtonElement>("guide-next").disabled = !ok;
  showGuideAction(step);
  if (ok && !guide.wasReady) moveGuide(1);
  else guide.wasReady = ok;
}

function showGuideAction(step: Step) {
  setText("guide-action-state", t(ready(step) ? "✓ 켜짐" : "아직 꺼짐"));
  document.getElementById("guide-action-state")?.classList.toggle("ok", ready(step));
}

async function perform(action: string) {
  const [name, arg] = action.split(":");
  if (name === "tab") return select(arg as Page);
  if (name === "jump") return element("scroll").scrollTo({ top: element(arg).offsetTop - 4, behavior: "smooth" });
  if (name === "guide") return startGuide(arg as GuideId);
  if (name === "guide-next") return moveGuide(1);
  if (name === "guide-prev") return moveGuide(-1);
  if (name === "guide-skip") return moveGuide(1, true);
  if (name === "guide-close") return closeGuide();
  if (name === "discord-link") return changeDiscord();
  if (name === "macro-toggle") return changeMacro();

  if (busy) return;
  busy = true;
  ++request;
  const previous = { ...overlay };
  try {
    if (name === "lang" || name === "toggle-ui" || name === "reset-overlay") {
      if (name === "lang") overlay.lang = arg as Language;
      if (name === "toggle-ui") overlay.visible = !overlay.visible;
      await syncOverlay(name === "reset-overlay");
      if (!overlay.visible && macroActive()) await changeMacro();
      pageMemory.set(page, element("scroll").scrollTop);
      render();
      if (name === "reset-overlay") message("조작 창 위치를 왼쪽 위로 되돌림");
    } else if (name === "biome") {
      await setup("biome", { action: arg });
    } else if (name === "webhook-test") {
      await invoke("biome_test");
      message("테스트 전송 대기열에 추가됨");
    } else if (name === "update") {
      update = await serviceAction("update_" + arg);
      message(String(update.message ?? (arg === "check" ? update.available ? "새 업데이트가 있습니다." : "최신 버전입니다." : "완료")));
      applyStatus();
    } else if (name === "task" || name === "go" || name === "mark" || name === "merchant" || name === "template" || name === "pop-test") {
      await macroSaves;
      const payload: Bag = (name === "go" || name === "mark") ? { destination: arg } : name === "merchant" ? { name: arg } : name === "template" || name === "pop-test" ? { biome: arg } : arg === "mcraft_test" ? { name: group("mcraft").potion } : {};
      const action = name === "go" ? "go" : name === "mark" ? "waypoint_mark" : name === "merchant" ? "mmerch_buy_test" : name === "template" ? "mpop_default" : name === "pop-test" ? "mpop_test" : arg;
      const result = await automationAction(action, payload);
      if (name === "template") { await loadMacro(); renderPage(); }
      message(String(result.message ?? result.info ?? result.engine ?? (name === "template" ? "기본 템플릿을 불러왔습니다." : "작업을 요청했습니다. 게임 위 Acrux 창에서 진행 상태를 확인하세요.")));
    } else if (name === "position") {
      const [feat, key] = arg.split(".");
      await macroSaves;
      await invoke("pick_macro_position", { feat, key, region: key.endsWith("_region") });
      await loadMacro();
      renderPage();
    } else if (name === "template-add" || name === "template-delete") {
      const [biome, index] = arg.split(".");
      await saveMacro(next => {
        const pop = next.mpop as Bag;
        const templates = pop.templates as Bag;
        const template = templates[biome] as Bag;
        const items = template.items as Bag[];
        if (name === "template-add") items.push({ name: "", amount: "ALL", min_have: 1 });
        else items.splice(Number(index), 1);
      });
      renderPage();
    } else if (name === "clear-navigation") {
      await invoke("clear_navigation");
      await loadMacro();
      renderPage();
    } else if (name === "reset") {
      current = await invoke<Status>("reset", { kind: arg as Reset });
      applyStatus();
    } else if (name === "capture") {
      await setup(current?.captureReady && !guide ? "stopCapture" : "capture");
    } else {
      await setup(name as Setup);
    }
  } catch (error) {
    overlay = previous;
    message(String(error));
  } finally {
    busy = false;
    void refresh();
  }
}

document.addEventListener("click", event => {
  const target = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-action]");
  if (target?.dataset.action && !target.disabled) void perform(target.dataset.action);
});
element<HTMLDialogElement>("guide").addEventListener("cancel", event => {
  event.preventDefault();
  closeGuide();
});

const mobileWindow = window as Window & { acruxMobileBack?: () => boolean };
mobileWindow.acruxMobileBack = () => {
  if (guide) closeGuide();
  else if (page !== "home") select("home");
  else return false;
  return true;
};

let numberSaves = Promise.resolve();
document.addEventListener("input", event => {
  const input = event.target as HTMLInputElement;
  const tuning = input.dataset.tuning as keyof Tuning | undefined;
  if (input.disabled || input.type !== "number" || !tuning || !Number.isFinite(input.valueAsNumber) || !input.validity.valid) return;
  const value = input.valueAsNumber;
  numberSaves = numberSaves.then(async () => {
    busy = true;
    ++request;
    try {
      if (current) {
        await changeTuning(tuning, value);
      }
    } catch (error) { message(String(error)); }
    finally { busy = false; }
  });
});

async function changeMacroSetting(input: HTMLInputElement) {
  if (input.disabled || !input.dataset.macro && !input.dataset.buy && !input.dataset.template) return;
  if (input.type === "number" && (!Number.isFinite(input.valueAsNumber) || !input.validity.valid)) { message("입력 범위를 확인하세요."); return; }
  const value: unknown = input.type === "checkbox" ? input.checked : input.type === "number" ? input.valueAsNumber : input.value.trim();
  ++request;
  try {
    await saveMacro(next => {
      if (input.dataset.macro) {
        const path = input.dataset.macro.split(".");
        let target = next;
        for (const key of path.slice(0, -1)) { if (!target[key] || typeof target[key] !== "object") target[key] = {}; target = target[key] as Bag; }
        target[path.at(-1)!] = value;
        if (path.length === 1 && ["useMove", "useSell"].includes(path[0])) (next.mfish as Bag)[path[0]] = value;
      } else if (input.dataset.buy) {
        const buy = (next.mmerch as Bag).buy as Bag;
        if (value) buy[input.dataset.buy] = 1; else delete buy[input.dataset.buy];
      } else {
        const [biome, index, field] = input.dataset.template!.split(".");
        const template = ((next.mpop as Bag).templates as Bag)[biome] as Bag;
        if (index === "mode") template.mode = value;
        else {
          if (field === "amount" && value !== "ALL" && (!/^\d+$/.test(String(value)) || Number(value) < 1)) throw new Error("사용 수량은 양의 정수 또는 ALL로 입력하세요.");
          (template.items as Bag[])[Number(index)][field] = field === "amount" && value !== "ALL" ? Number(value) : value;
        }
      }
    });
    applyStatus();
  } catch (error) { message(String(error)); renderPage(); }
}

document.addEventListener("change", async event => {
  const input = event.target as HTMLInputElement;
  if (input.dataset.macro || input.dataset.buy || input.dataset.template) return changeMacroSetting(input);
  const key = input.dataset.tuning as keyof Tuning | undefined;
  if (input.disabled || input.dataset.macro || input.dataset.buy || input.dataset.template || input.dataset.navigation || input.type === "number" && key) return;
  if (busy) return;
  if (input.dataset.biome) {
    busy = true;
    ++request;
    try {
      const field = input.dataset.biome;
      const value = field === "webhooks" ? input.value.split(/[\s,]+/).filter(Boolean)
        : input.type === "checkbox" ? input.checked : input.value.trim();
      biome = await saveBiome({ ...biome.settings, [field]: value });
      await setup("biome", { action: "refresh" });
    } catch (error) { message(String(error)); }
    finally { busy = false; void refresh(); }
  } else if (key && current) {
    const value = input.type === "checkbox" ? input.checked : input.valueAsNumber;
    if (input.type === "number" && (!Number.isFinite(value) || !input.validity.valid)) {
      input.value = String(current.tuning[key]);
      return;
    }
    busy = true;
    ++request;
    try {
      await changeTuning(key, value);
    } catch (error) {
      if (input.type === "checkbox") input.checked = !!current.tuning[key];
      else input.value = String(current.tuning[key]);
      message(String(error));
    } finally {
      busy = false;
      void refresh();
    }
  } else if (input.id === "online-share") {
    try {
      await invoke("save_online", { share: input.checked });
      online = await invoke<typeof online>("online_status");
      applyStatus();
    } catch (error) { input.checked = online.share; message(String(error)); }
  } else if (input.id === "opacity" || input.id === "hide-on-app") {
    const previous = { ...overlay };
    if (input.id === "opacity") overlay.alpha = input.valueAsNumber;
    else overlay.hideOnApp = input.checked;
    try { await syncOverlay(); } catch (error) {
      overlay = previous;
      message(String(error));
    }
  }
});

let touch: { x: number; y: number } | undefined;
element("pages").addEventListener("touchstart", event => {
  const first = event.touches[0];
  touch = (event.target as HTMLElement).closest("input, button, nav") ? undefined : { x: first.clientX, y: first.clientY };
}, { passive: true });
element("pages").addEventListener("touchend", event => {
  if (!touch) return;
  const point = event.changedTouches[0];
  const dx = point.clientX - touch.x;
  const dy = point.clientY - touch.y;
  touch = undefined;
  if (Math.abs(dx) < 60 || Math.abs(dx) < Math.abs(dy) * 1.5) return;
  const at = tabs.findIndex(([id]) => id === page) + (dx < 0 ? 1 : -1);
  if (tabs[at]) select(tabs[at][0]);
}, { passive: true });

render();
void refresh();
document.addEventListener("visibilitychange", () => { void refresh(); if (!document.hidden && page === "settings") void refreshDiscord(); });
window.setInterval(() => void refresh(), 500);
