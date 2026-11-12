import { invoke } from "@tauri-apps/api/core";
import { control, saveTuning, status, type Status, type Stats, type Tuning } from "./engine";
import mobileDictionary from "../android/app/src/main/assets/i18n/mobile.tsv?raw";
import desktopDictionary from "../android/app/src/main/assets/i18n/pc.tsv?raw";

import { biomeStatus, saveBiome, defaultBiome, type BiomeStatus } from "./biome";

type Language = "ko" | "en" | "ja";
type Page = "home" | "fish" | "biome" | "log" | "settings";
type Overlay = { visible: boolean; hideOnApp: boolean; alpha: number; lang: Language };
type Setup = "biome" | "accessibility" | "capture" | "stopCapture" | "appInfo" | "donate" | "overlay";
type Reset = "calibration" | "session" | "total" | "log";
type GuideId = "ready" | "fish" | "move" | "sell";

const translations = new Map<string, string[]>();
for (const dictionary of [desktopDictionary, mobileDictionary]) {
  for (const line of dictionary.split(/\r?\n/)) {
    if (!line || line.startsWith("#")) continue;
    const [ko, ...other] = line.split("\t");
    translations.set(ko, other.map(value => value.replaceAll("\\n", "\n")));
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
const unsupported = "이 기능의 실행은 아직 Tauri 미리보기에 연결되지 않았습니다.";
interface NavigationOptions {
  useMove: boolean; useSell: boolean; resetWait: number; wa: number; a: number; aw: number;
  pause: number; d: number; tilt: number; zoom: number; margin: number; sellMax: number; sellDelay: number;
}
interface NavigationState {
  options: NavigationOptions;
  positions: Record<string, [number, number]>;
  fish: number;
  sell: number;
}
const navigationDefaults: NavigationOptions = {
  useMove: false, useSell: false, resetWait: 3.5, wa: 3.5, a: 3, aw: 2, pause: .5,
  d: 1, tilt: 3, zoom: 4, margin: .3, sellMax: 100, sellDelay: 0,
};
let nav: NavigationState = { options: navigationDefaults, positions: {}, fish: 0, sell: 0 };
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

function button(title: string, action: string, color = "muted", id = "") {
  return `<button type="button" class="button ${color}" data-action="${action}" ${id ? `id="${id}"` : ""}>${tr(title)}</button>`;
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
    + text("③ 이동 · 판매: 로블록스 설정 → 이동 방식을 '조이스틱' (클래식) 으로", 12, "warning"))
  + card("state", "상태",
    '<p id="run-state" class="readout"></p><p id="cal-state" class="muted readout"></p>'
    + `<div class="buttons">${button("보정 초기화", "reset:calibration", "danger")}</div>`)
  + card("how", "사용법",
    [
      "1. 로블록스(Sol's RNG)를 전체 화면 가로로 켜고 낚시 자리로 가서 Fish 버튼이 보이게 하기",
      "2. 떠 있는 Acrux 창 ⚙ → [낚시] → 낚시 자동 보정 — Fish 를 한 번 던져 대기 창 · 미니게임 창 · 결과창 위치를 자동으로 맞춤",
      "3. [시작] — 자동 낚시 · 멈출 때는 [정지]",
      "4. Acrux 창은 어디든 잡고 끌어서 낚시 창 · 결과창을 가리지 않는 곳으로 옮기기",
      "5. 이동 · 판매: ⚙ → [조이스틱] 조이스틱 찾기 → [기준 장소] 공통 위치 → [길] 낚시 장소 · 판매 장소 길 기록 → [판매] 대화 버튼 · 판매 자동 보정",
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

const baseFields: [keyof NavigationOptions, string, string, number, number, number][] = [
  ["resetWait", "리셋 후 대기", "기본 3.5", .5, 30, .1],
  ["wa", "W + A", "기본 3.5", 0, 30, .1], ["a", "A", "기본 3", 0, 30, .1],
  ["aw", "A + W", "기본 2", 0, 30, .1], ["pause", "쉼", "D 누르기 전에 기다림 · 기본 0.5", 0, 10, .1],
  ["d", "D", "기본 1", 0, 30, .1], ["tilt", "내려다보기 (번)", "화면을 아래로 끄는 횟수 · 기본 3", 0, 10, 1],
  ["zoom", "줌 아웃 (번)", "두 손가락 오므리는 횟수 · 기본 4", 0, 10, 1],
  ["margin", "도착 뒤 여유", "길을 다 걸은 뒤 기다림 · 기본 0.3", 0, 5, .1],
];
const savedPlaces = [
  ["stick_edge", "조이스틱"], ["menu_btn", "로블록스 메뉴 버튼 (왼쪽 위)"],
  ["reset_btn", "캐릭터 재설정 버튼 (메뉴 안)"], ["reset_confirm", "재설정 확인 버튼"],
  ["collection_pos", "도감 버튼"], ["collection_close", "도감 Exit"], ["jump_btn", "점프 버튼 (오른쪽 아래)"],
  ["talk_btn", "대화 버튼 (E)"], ["dialog_pos", "대화창 (판매)"], ["sell_fish_pos", "Sell Fish 버튼 (판매)"],
  ["first_fish_pos", "첫 번째 물고기 칸 (판매)"], ["sell_all_pos", "Sell All 버튼 (판매)"],
  ["confirm_sell_pos", "확인 Sell (판매)"], ["shop_close_pos", "상점 닫기 X (판매)"],
];
function navigationField(key: keyof NavigationOptions, name: string, desc: string, min: number, max: number, step: number) {
  return row(name, desc, `<input type="number" data-navigation="${key}" aria-label="${tr(name)}" value="${nav.options[key]}" min="${min}" max="${max}" step="${step}">`);
}
function move() {
  return card("movement", "이동 · 판매",
    text("시작하면 재설정 → 기준 장소 → 기록한 길로 낚시 장소 · 인벤토리가 가득 차면 판매 장소에서 팔고 돌아옴 · 전부 화면 터치 (조이스틱 · 탭) · 위치 · 길은 Acrux 창 ⚙ 에서", 12)
    + toggle("시작할 때 낚시 장소로 이동", 'data-navigation="useMove"', nav.options.useMove)
    + toggle("가득 차면 판매하러 가기", 'data-navigation="useSell"', nav.options.useSell))
  + card("base", "기준 장소 (초)", baseFields.map(args => navigationField(...args)).join(""))
  + card("sell", "판매", navigationField("sellMax", "판매 최대 종류", "한 번에 파는 최대 종류 수 · 기본 100", 1, 500, 1)
    + navigationField("sellDelay", "판매 클릭 추가 대기 (초)", "렉이 있으면 늘림 · 기본 0", 0, 5, .1))
  + card("places", "저장된 위치 · 길", '<pre id="saved-places"></pre>'
    + `<div class="buttons">${button("위치 · 길 모두 지우기", "clear-navigation", "danger")}</div>`);
}
function applyNavigation() {
  const labels = savedPlaces.map(([key, label]) => {
    const at = nav.positions[key];
    return at ? "✓ " + t(label) + "  " + at.map(value => value.toFixed(4)).join(", ") : "· " + t(label);
  });
  labels.push(...([["fish", "낚시 장소 길"], ["sell", "판매 장소 길"]] as const)
    .map(([key, label]) => (nav[key] ? "✓ " : "· ") + t(label) + (nav[key] ? " · " + nav[key] + " " + t("조각") : "")));
  setText("saved-places", labels.join("\n"));
  for (const field of document.querySelectorAll<HTMLInputElement>("[data-navigation]")) {
    if (document.activeElement === field) continue;
    const value = nav.options[field.dataset.navigation as keyof NavigationOptions];
    if (field.type === "checkbox") field.checked = !!value;
    else field.value = String(value);
  }
}
function logs() {
  return card("recent", "기록", text("최근 기록 (최대 300줄)", 12)
    + `<pre id="records">${tr("아직 기록 없음")}</pre><div class="buttons">${button("기록 지우기", "reset:log")}</div>`);
}

function settings() {
  const version = current?.version || "0.1.0";
  return card("update", "업데이트",
    row("Acrux Mobile V" + version, `지금 V${version} · 새 버전이 나오면 받아서 바로 설치 (설정은 그대로)`, button("확인", "unsupported", "accent"))
    + toggle("앱을 켤 때 자동으로 업데이트", "", false, true))
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
  document.getElementById("access-state")?.classList.toggle("ok", !!s?.inputReady);
  document.getElementById("capture-state")?.classList.toggle("ok", !!s?.captureReady);
  setText("run-state", t(s?.message === "대기 중" || !s ? "대기" : s.message) + "\n"
    + t("성공 {0} · 쓰레기 {1} · 실패 {2}").replace("{0}", String(session.success)).replace("{1}", String(session.junk)).replace("{2}", String(session.fail))
    + "  (" + t("이번 실행") + ")");
  setText("cal-state", `${t("대기 창")}: ${t(s?.calibrated ? "있음" : "없음")}  ·  ${t("미니게임 창")}: ${t(s?.calibrated ? "대기 창으로 계산" : "없음")}  ·  ${t("결과창")}: ${t("없음")}`
    + (s?.calibrated ? "" : "\n→ " + t("로블록스에서 ⚙ → [낚시] → 낚시 자동 보정을 해야 함")));
  statsText("run", session);
  statsText("total", s?.totals ?? zero);
  if (page === "fish") applyNavigation();
  setText("online-count", online.online === null ? "--" : String(online.online));
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

async function refresh() {
  if (document.hidden || busy) return;
  const ticket = ++request;
  try {
    const next = await status();
    biome = await biomeStatus();
    transport = await invoke<typeof transport>("biome_transport");
    online = await invoke<typeof online>("online_status");
    const nextNavigation = page === "fish" || guide?.id === "move" || guide?.id === "sell"
      ? await invoke<NavigationState>("navigation_status") : undefined;
    if (ticket !== request) return;
    const initial = !current;
    current = next;
    if (nextNavigation) nav = nextNavigation;
    if (initial) {
      render();
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
  check?: "inputReady" | "captureReady" | "calibrated" | "movement";
  action?: "accessibility" | "capture";
  optional?: boolean;
  position?: string;
  route?: "fish" | "sell";
}

const guideSteps: Record<GuideId, Step[]> = {
  ready: [
    { title: "Acrux Mobile 에 오신 걸 환영합니다.", body: "이 매크로는 Sol's RNG 전용 매크로이며, 다양한 모바일 기능을 지원합니다.\n\n시작하기 전에, 매크로가 작동할 때 필요한 필수 권한 두 개를 켜야 합니다." },
    { title: "접근성 권한 켜기", body: "매크로가 로블록스 화면을 터치하는 데 필요한 권한입니다.\n\n설정 → 접근성 → 설치된 앱 → Acrux Mobile → 켜기\n\n'제한된 설정' 으로 켜지지 않는다면, 설정 → 앱 → Acrux Mobile → 오른쪽 위 ⋮ → '제한된 설정 허용' 후 다시 켜 주세요.", check: "inputReady", action: "accessibility" },
    { title: "화면 캡처 시작", body: "매크로가 화면을 보고 낚시 상태와 버튼을 찾는 데 필요합니다. 화면은 녹화되거나 저장되지 않습니다.\n\n공유 창이 뜨면 '전체 화면' 을 선택해 주세요.", check: "captureReady", action: "capture" },
    { title: "로블록스 준비", body: "· 로블록스 (Sol's RNG) 를 가로 전체 화면으로 실행합니다.\n· 로블록스 설정 → 이동 방식을 '조이스틱' (클래식) 으로 바꿉니다.\n\n로블록스 위에는 작은 Acrux 창이 표시되며, 끌어서 옮길 수 있습니다. 이 안내도 그 옆에 이어서 표시됩니다." },
    { title: "준비 완료", body: "이제 로블록스를 열어 주세요. 다음 안내 (낚시 보정) 가 이어서 표시됩니다." },
  ],
  fish: [
    { title: "낚시 보정", body: "낚시 창 · 미니게임 창 · 결과창의 위치를 이 기기 화면에 맞춥니다. 처음 한 번만 하면 됩니다." },
    { title: "낚시 자리로 이동", body: "로블록스에서 낚시 자리로 이동해 Fish 버튼이 보이게 해 주세요.\n\nAcrux 창이 낚시 창을 가린다면 위쪽 구석으로 옮겨 주세요." },
    { title: "낚시 자동 보정", body: "Acrux 창 ⚙ → [낚시] → 강조된 '낚시 자동 보정' 을 누르고 끝날 때까지 기다려 주세요. Fish 를 한 번 던져 창 위치를 자동으로 맞춥니다.\n\n보정하는 동안에는 이 안내가 잠시 숨겨집니다.", check: "calibrated" },
    { title: "낚시 보정 완료", body: "Acrux 창의 [시작] 을 누르면 자동 낚시가 시작되며, [정지] 로 멈출 수 있습니다.\n\n가득 찼을 때 판매까지 하려면 앱의 낚시 칸 → 이동 · 판매에서 켜 주세요." },
  ],
  move: [
    { title: "이동 설정", body: "스폰 위치가 무작위이기 때문에, 리셋 후 '기준 장소' 에서 기록한 길을 따라 이동합니다.\n\n조이스틱 → 기준 장소 위치 → 낚시 장소 길 순서로 설정합니다." },
    { title: "조이스틱 찾기", body: "Acrux 창 ⚙ → [조이스틱] → 강조된 '조이스틱 찾기' 를 눌러 주세요.\n\n로블록스 이동 방식이 '조이스틱' 이어야 합니다. 자동으로 찾지 못하면 '조이스틱 직접 지정' 을 사용할 수 있습니다.", check: "movement", position: "stick_edge" },
    { title: "점프 버튼 위치", body: "길 기록에서 점프를 쓰려면 설정해 주세요. 위치를 정할 때는 십자 포인터가 표시되며, 화면을 끌면 그만큼 움직이고 [확인] 으로 저장됩니다.\n\n점프를 쓰지 않는다면 건너뛰어도 됩니다.", check: "movement", position: "jump_btn", optional: true },
    ...savedPlaces.slice(1, 6).map(([position, name]) => ({
      title: "'" + name + "' 위치",
      position,
      body: "⚙ → [기준 장소] → 강조된 항목을 누르고, 십자 포인터를 해당 버튼 위에 맞춘 뒤 [확인] 을 눌러 주세요.\n\n메뉴 · 재설정 화면을 직접 열어 둔 상태에서 맞추면 더 정확합니다.",
      check: "movement" as const,
    })),
    { title: "낚시 장소 길 기록", body: "⚙ → [길] → 낚시 장소 '길 기록' 을 눌러 주세요.\n\n기준 장소로 이동한 뒤 WASD 패드가 표시됩니다.\n· W A S D 탭 = 켜기 · 다시 탭 = 끄기 · 모서리 = 대각선 · ■ = 멈춤\n· 점프 탭 = 한 번 · [홀드] = 계속 누름\n\n도착하면 Acrux 창의 [도착] 을 눌러 주세요.", check: "movement", route: "fish" },
    { title: "이동 설정 완료", body: "[시작] 을 누르면 기준 장소 → 낚시 장소로 이동한 뒤 낚시를 시작합니다.\n\n⚙ → [길] → '기준 장소 → 낚시 장소' 로 미리 확인할 수 있습니다." },
  ],
  sell: [
    { title: "판매 설정", body: "인벤토리가 가득 차면 판매 장소로 이동해 물고기를 판매한 뒤 돌아옵니다.\n\n판매 장소 길 → 대화 버튼 → 판매 자동 보정 순서로 설정합니다. [ 이동 설정이 먼저 필요합니다 ]" },
    { title: "판매 장소 길 기록", body: "⚙ → [길] → 물고기 판매 장소 '길 기록' 을 누른 뒤, 기준 장소에서 Captain Flarg 앞까지 WASD 패드로 이동하고 [도착] 을 눌러 주세요.", check: "movement", route: "sell" },
    { title: "대화 버튼 위치", body: "판매 장소에 선 상태로 ⚙ → [판매] → 강조된 '대화 버튼' 을 누르고, 포인터를 화면의 대화 버튼에 맞춘 뒤 [확인] 을 눌러 주세요.", check: "movement", position: "talk_btn" },
    { title: "판매 자동 보정", body: "⚙ → [판매] → '판매 자동 보정' 을 누른 뒤, Captain Flarg 앞에서 대화 버튼을 직접 눌러 주세요. 대화창 · 상점 · 확인창의 위치를 자동으로 찾습니다.\n\n확인창 위치까지 찾으려면 물고기를 가진 상태에서 진행해 주세요.", check: "movement" },
    { title: "판매 테스트", body: "⚙ → [판매] → '판매 한 번 해 보기' 로 판매가 끝까지 되는지 확인할 수 있습니다. 건너뛰어도 됩니다.", optional: true },
    { title: "판매 설정 완료", body: "[시작] 을 누르면 낚시 중 인벤토리가 가득 찼을 때 판매하고 돌아옵니다." },
  ],
};
let guide: { id: GuideId; index: number; wasReady: boolean } | null = null;

function ready(step: Step) {
  if (step.position) return !!nav.positions[step.position];
  if (step.route) return nav[step.route] > 0;
  if (step.check === "movement") return savedPlaces.slice(8).every(([key]) => nav.positions[key]);
  return !step.check || !!current?.[step.check];
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
    ${step.check === "movement" ? text("Roblox 화면의 Acrux 조작 창에서 설정하세요.", 12, "warning") : ""}
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
    if (id === "ready" && !current?.calibrated) startGuide("fish");
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
  if (name === "unsupported") return message(unsupported);
  if (name === "discord-link") return changeDiscord();

  if (busy) return;
  busy = true;
  ++request;
  const previous = { ...overlay };
  try {
    if (name === "lang" || name === "toggle-ui" || name === "reset-overlay") {
      if (name === "lang") overlay.lang = arg as Language;
      if (name === "toggle-ui") overlay.visible = !overlay.visible;
      await syncOverlay(name === "reset-overlay");
      if (!overlay.visible && current?.phase !== "idle") current = await control("stop");
      pageMemory.set(page, element("scroll").scrollTop);
      render();
      if (name === "reset-overlay") message("조작 창 위치를 왼쪽 위로 되돌림");
    } else if (name === "biome") {
      await setup("biome", { action: arg });
    } else if (name === "webhook-test") {
      await invoke("biome_test");
      message("테스트 전송 대기열에 추가됨");
    } else if (name === "clear-navigation") {
      await invoke("clear_navigation");
      nav = await invoke<NavigationState>("navigation_status");
      applyNavigation();
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
  const movement = input.dataset.navigation as keyof NavigationOptions | undefined;
  if (input.type !== "number" || (!tuning && !movement) || !Number.isFinite(input.valueAsNumber) || !input.validity.valid) return;
  const value = input.valueAsNumber;
  numberSaves = numberSaves.then(async () => {
    busy = true;
    ++request;
    try {
      if (movement) {
        const options = { ...nav.options, [movement]: value };
        await invoke("save_navigation", { options });
        nav = { ...nav, options };
      } else if (tuning && current) {
        current = await saveTuning({ ...current.tuning, [tuning]: value });
      }
    } catch (error) { message(String(error)); }
    finally { busy = false; }
  });
});

document.addEventListener("change", async event => {
  const input = event.target as HTMLInputElement;
  const key = input.dataset.tuning as keyof Tuning | undefined;
  const movement = input.dataset.navigation as keyof NavigationOptions | undefined;
  if (input.type === "number" && (key || movement)) return;
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
  } else if (movement) {
    busy = true;
    ++request;
    try {
      const value = input.type === "checkbox" ? input.checked : input.valueAsNumber;
      if (input.type === "number" && (!Number.isFinite(value) || !input.validity.valid)) return;
      const options = { ...nav.options, [movement]: value };
      await invoke("save_navigation", { options });
      nav = { ...nav, options };
      if (value && movement === "useMove" && !nav.fish) startGuide("move");
      if (value && movement === "useSell" && !nav.sell) startGuide(nav.fish ? "sell" : "move");
    } catch (error) {
      message(String(error));
    } finally {
      busy = false;
      if (input.type === "checkbox") input.checked = !!nav.options[movement];
      else input.value = String(nav.options[movement]);
      void refresh();
    }
  } else if (key && current) {
    const value = input.type === "checkbox" ? input.checked : input.valueAsNumber;
    if (input.type === "number" && (!Number.isFinite(value) || !input.validity.valid)) {
      input.value = String(current.tuning[key]);
      return;
    }
    busy = true;
    ++request;
    try {
      current = await saveTuning({ ...current.tuning, [key]: value });
    } catch (error) {
      if (input.type === "checkbox") input.checked = !!current.tuning[key];
      else input.value = String(current.tuning[key]);
      message(String(error));
    } finally {
      busy = false;
      void refresh();
    }
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
