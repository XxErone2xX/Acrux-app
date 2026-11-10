'use strict';

// ---------------------------------------------------------------- 서버 연결
const TOKEN = location.hash.slice(1);
history.replaceState(null, '', location.pathname);

// timeout: 이 시간(ms) 안에 응답이 없으면 실패로 봄 (상태 받기 등) — 위치 지정처럼 사용자를 기다리는 요청은 안 씀
async function api(name, payload = {}, timeout = 0) {
  if (window.acruxDesktopApi) return window.acruxDesktopApi(name, payload);
  const ctl = timeout ? new AbortController() : null;
  const t = ctl && setTimeout(() => ctl.abort(), timeout);
  try {
    const r = await fetch('/api/' + name, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Token': TOKEN },
      body: JSON.stringify(payload),
      signal: ctl?.signal,
    });
    if (!r.ok) throw new Error(name + ' ' + r.status);
    return await r.json();
  } finally { clearTimeout(t); }
}

// ---------------------------------------------------------------- 브라우저 기능 막기
// 화면이 Edge 앱 창이라, F3 · Ctrl+F(찾기) · Ctrl+P(인쇄) · Ctrl+S(저장) · Ctrl+U(소스) · F5 · Ctrl+R(새로고침) · F12(개발자 도구) ·
// Ctrl + 휠(확대) · 우클릭 메뉴 · 마우스 뒤로 가기 같은 브라우저 기능이 뜨지 않게 막음
// (F3 · F6 · F7 은 매크로 단축키라 게임 중에도 자주 눌림 · 입력칸에서는 복사 · 붙여넣기 등 편집 단축키와 우클릭 메뉴는 그대로)
(function blockBrowserUi() {
  const editing = t => !!t?.closest?.('input, textarea, select, [contenteditable="true"]');
  const EDIT_KEYS = new Set(['a', 'c', 'v', 'x', 'z', 'y']);
  addEventListener('keydown', e => {
    const k = String(e.key || ''), low = k.toLowerCase();
    let block = /^F([1-9]|1[0-2])$/.test(k);                       // F1 ~ F12
    if (e.ctrlKey || e.metaKey) {
      block = !(EDIT_KEYS.has(low) && !e.altKey && (editing(e.target) || low === 'c'));   // 편집 단축키 · 복사만 허용
    }
    if (e.altKey && ['ArrowLeft', 'ArrowRight', 'Home'].includes(k)) block = true;      // 뒤로 · 앞으로 · 홈
    if (k === 'BrowserBack' || k === 'BrowserForward' || k === 'BrowserRefresh' || k === 'BrowserSearch') block = true;
    if (block) e.preventDefault();
  }, true);
  addEventListener('contextmenu', e => { if (!editing(e.target)) e.preventDefault(); }, true);
  addEventListener('wheel', e => { if (e.ctrlKey) e.preventDefault(); }, { capture: true, passive: false });
  for (const type of ['mouseup', 'mousedown', 'auxclick'])                         // 마우스 옆 버튼(뒤로 · 앞으로)
    addEventListener(type, e => { if (e.button === 3 || e.button === 4) e.preventDefault(); }, true);
  addEventListener('dragstart', e => { if (!editing(e.target)) e.preventDefault(); }, true);   // 이미지 · 글자 끌어다 놓기
})();

// ---------------------------------------------------------------- 아이콘 (직접 그린 SVG)
const ICONS = {
  play: '<svg viewBox="0 0 24 24"><path d="M7 4l13 8-13 8z" fill="currentColor" stroke="none"/></svg>',
  stop: '<svg viewBox="0 0 24 24"><rect x="6" y="6" width="12" height="12" fill="currentColor" stroke="none"/></svg>',
  wait: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M12 7v5l3 2"/></svg>',
  back: '<svg viewBox="0 0 24 24"><path d="M15 5l-7 7 7 7"/></svg>',
  chev: '<svg viewBox="0 0 24 24"><path d="M9 5l7 7-7 7"/></svg>',
  chat: '<svg viewBox="0 0 24 24"><path d="M4 5h16v11H9l-5 4z"/><path d="M8 9h8M8 12h5"/></svg>',
  bolt: '<svg viewBox="0 0 24 24"><path d="M13 3L5 14h6l-1 7 8-11h-6z"/></svg>',
  undo: '<svg viewBox="0 0 24 24"><path d="M9 7L4 12l5 5"/><path d="M4 12h11a5 5 0 010 10h-3"/></svg>',
  x: '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>',
  log: '<svg viewBox="0 0 24 24"><path d="M5 4h14v16H5z"/><path d="M8 9h8M8 13h8M8 17h5"/></svg>',
  gift: '<svg viewBox="0 0 24 24"><path d="M4 10h16v10H4zM3 7h18v3H3zM12 7v13"/><path d="M12 7c-1.5-3-5-3-5-1s3 1 5 1zM12 7c1.5-3 5-3 5-1s-3 1-5 1z"/></svg>',
  heart: '<svg viewBox="0 0 24 24"><path d="M12 20s-7-4.4-7-10a4 4 0 017-2.6A4 4 0 0119 10c0 5.6-7 10-7 10z"/></svg>',
  trash: '<svg viewBox="0 0 24 24"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6"/></svg>',
  shield: '<svg viewBox="0 0 24 24"><path d="M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6z"/><path d="M9 12l2 2 4-4"/></svg>',
  pin: '<svg viewBox="0 0 24 24"><path d="M12 21s-6-5.6-6-11a6 6 0 0112 0c0 5.4-6 11-6 11z"/><circle cx="12" cy="10" r="2.2"/></svg>',
  chart: '<svg viewBox="0 0 24 24"><path d="M4 20h16"/><path d="M7 16v-5M12 16V6M17 16v-8"/></svg>',
  notes: '<svg viewBox="0 0 24 24"><path d="M6 3h9l4 4v14H6z"/><path d="M14 3v5h5M9 12h7M9 16h7"/></svg>',
  gear: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/></svg>',
  globe: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M4 12h16M12 4c2.5 2.5 2.5 13.5 0 16M12 4c-2.5 2.5-2.5 13.5 0 16"/></svg>',
};
const icon = n => `<span class="ico">${ICONS[n] || ''}</span>`;

function addDeco(el) {
  if (el.querySelector(':scope > .c')) return;
  for (const k of ['tl', 'tr', 'bl', 'br']) {
    const s = document.createElement('i');
    s.className = 'c ' + k;
    el.appendChild(s);
  }
}

// ---------------------------------------------------------------- 메뉴
// step: 순서대로 번호 (모드가 쓰는 것 · 예: 스나이핑 모드 ① 링크 감지 → ② 접속 → ③ 오토 팝핑 → ④ 복귀)
// 모드는 Mods.addMenu · Mods.addTab 으로 칸 · 탭을 더함 (아래 '모드 · 플러그인')
// grad: 아이콘 칸 그라데이션 / decos: ㄱ 장식 네 모서리 색 [왼위, 오위, 왼아래, 오아래] / cls: 버튼에 붙일 이름
const MENU = [
  { key: 'biome', tab: 'biome', icon: 'globe', color: 'var(--cyan)', title: '바이옴 매크로 설정', desc: '바이옴 알림 · 디스코드 웹후크 · 멘션' },
  // 매크로 탭: 예전 기능 설정 + 기준 위치 설정을 기능별로 합침 (기능마다 켜기 · 설정 · 위치 · 이동을 한 칸에)
  { key: 'mfeat', tab: 'macro', icon: 'bolt', color: 'var(--yellow)', title: '매크로 설정', desc: '공통 위치 · 기능마다 켜기 · 설정 · 위치 · 이동' },
  // 복구 · 게임 입장: 로블록스가 꺼졌을 때 내 서버로 · Play 버튼 (모드가 다른 서버에 들어갈 때도 씀)
  { key: 'recover', tab: 'macro', icon: 'undo', color: 'var(--green)', title: '복구 · 게임 입장', desc: '로블록스가 꺼지면 내 서버로 · Play 버튼 위치' },
  { key: 'acrux', tab: 'acrux', icon: 'gear', color: '#7c6cf6', title: 'Acrux 설정', desc: '언어 · OCR 감지 방식 · 데이터 폴더',
    grad: 'linear-gradient(135deg, #8fa0ff, #6c7bff 50%, #8b5cf6)' },
  { key: 'updates', tab: 'acrux', icon: 'notes', color: 'var(--green)', title: '업데이트 로그', desc: '지금까지 바뀐 점 · 맨 위가 최신' },
  { key: 'mstats', tab: 'acrux', icon: 'chart', color: 'var(--cyan)', title: '통계', desc: '이번 실행 · 올타임 기록' },
  { key: 'mods', tab: 'extra', icon: 'bolt', color: '#e8a33d', title: '모드 · 플러그인', desc: '설치한 모드 · 켜기 · 끄기 · 모드 파일 설치',
    grad: 'linear-gradient(135deg, #f7c548, #e8a33d 55%, #b4651c)' },
];
// 설정 탭 — 끝없이 돌아감 (바이옴 → (모드 탭) → 매크로 → 추가 기능 → Acrux → 바이옴 …)
const TABS = ['biome', 'macro', 'extra', 'acrux'];
const TAB_SCALE = 1.65, TAB_GAP = 40;     // 지금 탭 확대 배율 · 옆 탭과 보이는 간격(px)
let curTab = 'macro';
const wrapIdx = n => (n % TABS.length + TABS.length) % TABS.length;
function setTab(tab, animate = true, dir = 0) {
  if (!TABS.includes(tab)) tab = 'macro';
  const prev = curTab;
  curTab = tab;
  const k = TABS.indexOf(tab);
  if (!dir && prev !== tab) {        // 방향을 안 정했으면 가까운 쪽으로
    const d = wrapIdx(k - TABS.indexOf(prev));
    dir = d === 1 ? 1 : -1;
  }
  const menu = $('menu');
  menu.querySelectorAll('.menu-tab').forEach(p => {
    const on = p.dataset.tab === tab, was = p.classList.contains('on');
    if (on && !was && animate && dir) {
      // 들어오는 칸: 넘기는 방향 쪽에서 출발
      p.classList.add('no-anim');
      p.classList.remove('left', 'right');
      p.classList.add(dir > 0 ? 'right' : 'left');
      void p.offsetWidth;
      p.classList.remove('no-anim');
    }
    if (!on && was) {                // 나가는 칸: 반대쪽으로
      p.classList.remove('left', 'right');
      p.classList.add(dir > 0 ? 'left' : 'right');
    }
    p.classList.toggle('on', on);
    if (on) p.classList.remove('left', 'right');
  });
  // 이름 줄: 지금 탭은 가운데, 이전 탭은 왼쪽, 다음 탭은 오른쪽 (돌아가며)
  // 글자 길이가 달라도 보이는 간격이 양쪽 똑같게: 가운데 글자 끝에서 같은 거리만큼 떨어뜨림
  const onBtn = document.querySelector(`.tab-name[data-tab="${tab}"]`);
  // 글자는 큰 크기로 그려져 있음: 지금 탭은 그대로, 옆 탭은 1/배율로 줄어 보임
  const half = onBtn.offsetWidth / 2;
  const posX = (b, r) => r === 0 ? 0 : Math.sign(r) * (half + TAB_GAP + b.offsetWidth / TAB_SCALE / 2 + (Math.abs(r) > 1 ? 90 : 0));
  const setR = (b, r) => b.style.setProperty('--x', posX(b, r) + 'px');
  document.querySelectorAll('.tab-name').forEach(b => {
    const r = wrapIdx(TABS.indexOf(b.dataset.tab) - k + 1) - 1;      // -1 / 0 / 1
    const old = b._r;
    b._r = r;
    clearTimeout(b._t);
    b.classList.toggle('on', r === 0);
    b.classList.toggle('far', Math.abs(r) > 1);       // 탭이 4개 이상: 반대편 탭 이름은 숨김
    if (animate && old !== undefined && Math.abs(r - old) > 1) {
      // 반대편으로 넘어가는 이름: 가운데를 가로지르지 않고 바깥으로 빠졌다가 → 반대쪽 바깥에서 들어옴
      b.classList.add('out');
      setR(b, old * 2);
      b._t = setTimeout(() => {
        b.classList.add('no-anim');
        setR(b, r * 2);
        void b.offsetWidth;
        b.classList.remove('no-anim', 'out');
        setR(b, r);
      }, 170);
      return;
    }
    if (!animate) b.classList.add('no-anim');
    b.classList.remove('out');
    setR(b, r);
    if (!animate) { void b.offsetWidth; b.classList.remove('no-anim'); }
  });
  if (config && config.menu_tab !== tab) queueSave({ menu_tab: tab });
}
const stepTab = d => setTab(TABS[wrapIdx(TABS.indexOf(curTab) + d)], true, d);

const headHTML = m => `
  <div class="tile" style="background:${m.grad || `linear-gradient(135deg, color-mix(in srgb, ${m.color} 65%, white), ${m.color} 55%, color-mix(in srgb, ${m.color} 80%, black))`}">${icon(m.icon)}</div>
  <div><b>${m.step ? `<span class="flow-no" data-notr style="--fc:${m.color}">${m.step}</span>` : ''}<span>${m.title}</span>${m.soon ? '<span class="tag">준비 중</span>' : ''}</b><span class="desc">${m.desc}</span></div>`;

const LOG_ENTRY = { key: 'log', icon: 'log', color: 'var(--muted)', title: '로그', desc: '동작 기록' };
// grad: 아이콘 칸 그라데이션 / decos: ㄱ 장식 네 모서리 색 [왼위, 오위, 왼아래, 오아래]
const DONATE_ENTRY = { key: 'donate', icon: 'gift', color: '#eb9a2a', title: 'Donate', desc: '후원하기',
  grad: 'linear-gradient(135deg, #f7c548, #e67e22)', decos: ['#f5be42', '#e98a26', '#f0ac36', '#e67e22'] };
// 크레딧: 얼로니 닉네임 색 (청록 → 하늘 → 보라 → 분홍)
const CREDIT_ENTRY = { key: 'credit', icon: 'heart', color: '#a070f8', title: '크레딧', desc: '만든 사람 · 링크',
  grad: 'linear-gradient(135deg, #7ce0d8, #7cc8f8 35%, #a070f8 70%, #d078e8)', decos: ['#7ce0d8', '#a070f8', '#7cc8f8', '#d078e8'] };
// 피드백: 하늘색
const FEEDBACK_ENTRY = { key: 'feedback', icon: 'chat', color: '#3ba7e8', title: '피드백', desc: '디스코드 서버에서 버그 · 건의',
  grad: 'linear-gradient(135deg, #7cc8f8, #3ba7e8 55%, #2a7fc0)' };
// 완전 삭제: 빨간색
const UNINSTALL_ENTRY = { key: 'uninstall', icon: 'trash', color: '#da373c', title: '완전 삭제', desc: 'Acrux 데이터 전부 지우기',
  grad: 'linear-gradient(135deg, #f0656a, #da373c 55%, #a1282c)' };
const DECO_KEYS = ['--deco-tl', '--deco-tr', '--deco-bl', '--deco-br'];
function setDeco(el, m) {
  el.style.setProperty('--deco', m.color);
  DECO_KEYS.forEach((k, n) => m.decos ? el.style.setProperty(k, m.decos[n]) : el.style.removeProperty(k));
}

function setupPage(m) {
  const page = document.getElementById('page-' + m.key);
  setDeco(page, m);
  page.querySelector('[data-head]').innerHTML = headHTML(m);
  page.querySelectorAll('.side .item').forEach((it, i) => {
    it.style.setProperty('--i', i);
    it.style.setProperty('--deco', m.color);
  });
  page.style.setProperty('--n', page.querySelectorAll('.side .item').length);
}

function buildMenu() {
  const menu = document.getElementById('menu');
  for (const [sel, entry] of [['.log-open', LOG_ENTRY], ['.credit-open', CREDIT_ENTRY]]) {
    const btn = document.querySelector(sel);
    setDeco(btn, entry);
    btn.querySelector('.head').innerHTML = headHTML(entry);
    btn.addEventListener('click', () => openPage(entry.key));
    setupPage(entry);
  }
  // 왼쪽 아래 삼각형 버튼: 누르면 Donate·크레딧·로그 버튼이 위로 올라옴 (다시 누르면 내려감)
  const corner = document.querySelector('.corner'), tog = $('cornerToggle');
  tog.addEventListener('click', () => {
    const open = corner.classList.toggle('open');
    tog.setAttribute('aria-expanded', open);
    tog.title = open ? '접기' : '더 보기';
  });
  // 피드백: 글 쓰는 창 → 보내기 (만든 사람 디스코드 채널로)
  const fbBtn = document.querySelector('.feedback-open');
  setDeco(fbBtn, FEEDBACK_ENTRY);
  fbBtn.querySelector('.head').innerHTML = headHTML(FEEDBACK_ENTRY);
  // 피드백: 서포트 디스코드 서버로 연결 (크레딧의 디스코드와 같은 주소)
  fbBtn.addEventListener('click', () => api('open_web', { site: 'discord' }));
  // 완전 삭제: 확인 창을 띄우고, 확인하면 데이터 전부 삭제 후 창 닫힘
  const unBtn = document.querySelector('.uninstall-open');
  setDeco(unBtn, UNINSTALL_ENTRY);
  unBtn.querySelector('.head').innerHTML = headHTML(UNINSTALL_ENTRY);
  unBtn.addEventListener('click', () => openConfirm());
  // Donate: 크레딧 탭 밖 (메인 왼쪽 아래) — ko-fi.com/acrux_
  const donateBtn = document.querySelector('.donate-open');
  setDeco(donateBtn, DONATE_ENTRY);
  donateBtn.querySelector('.head').innerHTML = headHTML(DONATE_ENTRY);
  document.querySelectorAll('[data-web]').forEach(b => b.addEventListener('click', async () => {
    const r = await api('open_web', { site: b.dataset.web });
    if (r.error) toast(r.error);
  }));
  // 탭마다 칸을 따로 만들고 같은 자리에 겹쳐둠 (지금 탭만 보임)
  const panels = {};
  for (const t of TABS) {
    const p = document.createElement('div');
    p.className = 'menu-tab';
    p.dataset.tab = t;
    menu.appendChild(p);
    panels[t] = p;
  }
  document.querySelectorAll('.tab-name').forEach(b => b.addEventListener('click', () => setTab(b.dataset.tab)));
  for (const m of MENU) {
    const b = document.createElement('button');
    b.className = 'menu-card deco' + (m.cls ? ' ' + m.cls : '') + (m.step ? ' flow' : '');
    setDeco(b, m);
    b.dataset.key = m.key;
    b.innerHTML = `<div class="head">${headHTML(m)}</div>` +
      (m.toggle ? `<label class="switch card-switch" title="바이옴 매크로 켜기/끄기"><input type="checkbox" data-toggle="${m.key}"><i></i></label>` : '') +
      `<span class="chev">${icon('chev')}</span>`;
    b.addEventListener('click', e => { if (!e.target.closest('.card-switch')) openPage(m.key); });
    panels[m.tab].appendChild(b);
    setupPage(m);
  }
  document.querySelectorAll('[data-icon]').forEach(e => { e.innerHTML = ICONS[e.dataset.icon] || ''; });
  document.querySelectorAll('.deco').forEach(addDeco);
  document.querySelectorAll('[data-back]').forEach(b => b.addEventListener('click', closePage));
}

// ---------------------------------------------------------------- 언어 (Language)
// 메인 화면 왼쪽 위 + 튜토리얼 안내창(하위 화면에 있을 때) — 둘 다 같은 설정
function buildLang(box, compact) {
  box.innerHTML = `<button class="lang-btn" type="button" aria-expanded="false" data-notr>${icon('globe')}` +
    `${compact ? '' : '<span>Language</span>'}<b class="lang-cur"></b><span class="lang-caret"></span></button>` +
    `<div class="lang-menu" data-notr>${Object.entries(I18N.LANGS).map(([k, n]) =>
      `<button type="button" data-lang="${k}"><span class="lang-check"></span>${n}</button>`).join('')}</div>`;
  const btn = box.querySelector('.lang-btn');
  if (!compact) { btn.classList.add('deco'); addDeco(btn); }      // 다른 버튼처럼 ㄱ 장식
  btn.addEventListener('click', () => {
    const open = !box.classList.contains('open');
    closeLangMenus();
    box.classList.toggle('open', open);
    btn.setAttribute('aria-expanded', open);
  });
  box.querySelectorAll('[data-lang]').forEach(b => b.addEventListener('click', () => { closeLangMenus(); setLang(b.dataset.lang); }));
  const sync = l => {
    box.querySelector('.lang-cur').textContent = compact ? l.toUpperCase() : I18N.LANGS[l];
    box.querySelectorAll('[data-lang]').forEach(b => b.classList.toggle('on', b.dataset.lang === l));
  };
  I18N.onChange(sync);
  sync(I18N.lang);
}
function closeLangMenus() {
  document.querySelectorAll('.lang.open').forEach(b => { b.classList.remove('open'); b.querySelector('.lang-btn').setAttribute('aria-expanded', false); });
}
addEventListener('click', e => { if (!e.target.closest('.lang')) closeLangMenus(); });
addEventListener('keydown', e => { if (e.key === 'Escape') closeLangMenus(); });
function setLang(l) {
  if (l === I18N.lang) return;
  I18N.set(l);
  queueSave({ lang: l });            // 프로그램 쪽(위치 지정 창 안내 글자 등)도 같은 언어로
}

// ---------------------------------------------------------------- 펼침 / 접힘 애니메이션
// 열기: 누른 카드가 화면을 꽉 채우며 커지고, 카드 안 아이콘·제목은 위로 올라가고, 하위 항목이 주르륵
// 닫기: 하위 항목이 사라지고, 화면이 카드 모양으로 줄어들며 원래 자리로
const shell = document.getElementById('shell');
const fly = document.getElementById('flyHead');
const decoBox = document.getElementById('decoBox');
let current = null, busy = false;
const GROW_MS = 420, SHRINK_MS = 380;
const EASE = 'cubic-bezier(.65,0,.35,1)';
const css = v => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
// 화면 배율: 창이 기본 크기보다 작으면 UI 전체를 그만큼 축소 (기본 크기 이상이면 그대로)
const BASE_W = 900, BASE_H = 720;   // 이보다 작아질 때만 축소 (메인 화면 내용이 실제로 차지하는 크기 기준)
let Z = 1;
function applyScale() {
  Z = Math.max(0.5, Math.min(1, innerWidth / BASE_W, innerHeight / BASE_H));
  document.documentElement.style.zoom = Z;
}
applyScale();
addEventListener('resize', applyScale);
// 위치값: 화면에서 잰 값(getBoundingClientRect)은 배율이 적용된 값이라 스타일에 넣을 땐 배율로 나눔
const u = v => v / Z;
const box = r => ({ left: u(r.left) + 'px', top: u(r.top) + 'px', width: u(r.width) + 'px', height: u(r.height) + 'px' });
const fullBox = () => ({ left: '0px', top: '0px', width: u(innerWidth) + 'px', height: u(innerHeight) + 'px' });
const tr = r => `translate(${u(r.left)}px, ${u(r.top)}px)`;
const wait = ms => new Promise(res => setTimeout(res, ms));

function showDeco(card, rect) {
  const cs = getComputedStyle(card);
  for (const k of ['--deco', ...DECO_KEYS]) {
    const v = cs.getPropertyValue(k);
    if (v) decoBox.style.setProperty(k, v); else decoBox.style.removeProperty(k);
  }
  Object.assign(decoBox.style, box(rect), { display: 'block' });
}
function hideDeco() {
  decoBox.getAnimations().forEach(x => x.cancel());
  decoBox.style.display = 'none';
}

function showFly(html, rect) {
  fly.innerHTML = html;
  fly.style.display = 'flex';
  fly.style.transform = tr(rect);
}

async function openPage(key) {
  if (busy || current) return;
  busy = true;
  Tutorial.onNavigate();
  closeLangMenus();
  Quick.hide();
  document.body.classList.add('in-page');
  const card = document.querySelector(`[data-key="${key}"].menu-card`);
  const page = document.getElementById('page-' + key);
  const cardRect = card.getBoundingClientRect();
  const cardHead = card.querySelector('.head');
  const headFrom = cardHead.getBoundingClientRect();

  // 하위 화면은 안 보이게 깔아두고 머리 도착 위치만 잰다
  page.classList.remove('reveal', 'leaving');
  page.classList.add('show', 'entering', 'measure');
  const headTo = page.querySelector('[data-head]').getBoundingClientRect();
  const barTo = page.querySelector('.topbar').getBoundingClientRect();

  Object.assign(shell.style, box(cardRect), { display: 'block', background: css('--panel') });
  showDeco(card, cardRect);
  showFly(cardHead.innerHTML, headFrom);
  card.style.visibility = 'hidden';

  const a = shell.animate([{ ...box(cardRect), background: css('--panel') }, { ...fullBox(), background: css('--bg') }],
    { duration: GROW_MS, easing: EASE, fill: 'forwards' });
  fly.animate([{ transform: tr(headFrom) }, { transform: tr(headTo) }],
    { duration: GROW_MS, easing: EASE, fill: 'forwards' });
  // 버튼의 ㄱ 장식은 화면 전체가 아니라 탭 위 바로 옮겨감
  decoBox.animate([box(cardRect), box(barTo)], { duration: GROW_MS, easing: EASE, fill: 'forwards' });
  await a.finished;

  page.classList.remove('measure', 'entering');
  page.classList.add('reveal');
  hideDeco();
  if (key === 'log') scrollLog(true);
  if (key === 'mfeat') { renderMpop(); syncFeatItems(); }   // 플레이어 이름 · 버튼 위치가 바뀌었을 수 있음
  if (key === 'acrux') refreshOcrInfo();
  if (key === 'updates') loadChangelog();
  if (key === 'mstats') renderStats(true);
  shell.getAnimations().forEach(x => x.cancel());
  fly.getAnimations().forEach(x => x.cancel());
  shell.style.display = 'none';
  fly.style.display = 'none';
  current = key;
  if (!page.querySelector('.side .item.active')) selectSection(page, page.querySelector('.side .item')?.dataset.sec);
  const n = page.querySelectorAll('.side .item').length;
  await wait(n * 55 + 320);
  busy = false;
  Tutorial.onNavigated();
}

async function closePage() {
  if (busy || !current) return;
  Tutorial.onNavigate();
  busy = true;
  const key = current;
  const card = document.querySelector(`[data-key="${key}"].menu-card`);
  const page = document.getElementById('page-' + key);
  const n = page.querySelectorAll('.side .item').length;

  page.classList.remove('reveal');
  page.classList.add('leaving');
  await wait(n * 40 + 180);

  const headFrom = page.querySelector('[data-head]').getBoundingClientRect();
  const headTo = card.querySelector('.head').getBoundingClientRect();
  const cardRect = card.getBoundingClientRect();
  const barFrom = page.querySelector('.topbar').getBoundingClientRect();
  Object.assign(shell.style, fullBox(), { display: 'block', background: css('--bg') });
  showDeco(card, barFrom);
  showFly(page.querySelector('[data-head]').innerHTML, headFrom);
  page.classList.remove('show', 'leaving');

  const a = shell.animate([{ ...fullBox(), background: css('--bg') }, { ...box(cardRect), background: css('--panel') }],
    { duration: SHRINK_MS, easing: EASE, fill: 'forwards' });
  fly.animate([{ transform: tr(headFrom) }, { transform: tr(headTo) }],
    { duration: SHRINK_MS, easing: EASE, fill: 'forwards' });
  decoBox.animate([box(barFrom), box(cardRect)], { duration: SHRINK_MS, easing: EASE, fill: 'forwards' });
  await a.finished;

  card.style.visibility = '';
  hideDeco();
  shell.getAnimations().forEach(x => x.cancel());
  fly.getAnimations().forEach(x => x.cancel());
  shell.style.display = 'none';
  fly.style.display = 'none';
  current = null;
  document.body.classList.remove('in-page');
  busy = false;
  Tutorial.onNavigated();
}

document.addEventListener('keydown', e => { if (e.key === 'Escape') closePage(); });
// 메인 화면에서 ← → 로 설정 탭 넘기기 (입력칸에 있을 때 · 다른 화면 · 튜토리얼 중엔 안 함)
document.addEventListener('keydown', e => {
  if ((e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') || current || busy || Tutorial.isOpen()) return;
  if (e.target.closest?.('input, textarea, select')) return;
  stepTab(e.key === 'ArrowLeft' ? -1 : 1);
});

function unleave(x) {
  x.getAnimations({ subtree: true }).forEach(a => a.cancel());
  x.classList.remove('leaving');
  for (const k of ['top', 'left', 'width', 'height']) x.style[k] = '';
}
// 왼쪽 목록(탭)으로 칸을 바꿀 때: 나가는 칸은 잠깐 겹쳐 둔 채 흐려지며 빠지고,
// 들어오는 칸은 누른 방향(위 · 아래)에서 줄마다 조금씩 늦게 미끄러져 들어옴 (윈도우 애니메이션 효과 설정과 상관없이)
function selectSection(page, sec) {
  if (!sec) return;
  const items = [...page.querySelectorAll('.side .item')];
  const from = items.findIndex(i => i.classList.contains('active')), to = items.findIndex(i => i.dataset.sec === sec);
  const old = page.querySelector('.content .sec.active');
  const el = page.querySelector(`.content .sec[data-sec="${sec}"]`);
  const move = !!(el && old && el !== old && from >= 0 && from !== to);
  // 앞에서 빠지던 칸이 남아 있으면 정리 (빠르게 여러 번 누른 경우)
  page.querySelectorAll('.content .sec.leaving').forEach(unleave);
  if (move) {
    // 나가는 칸은 지금 있던 그 자리 그대로 겹쳐 둠 (칸 위에 다른 줄이 있는 화면도 · 예: 디스코드 감지 설정의 감시 대상 줄)
    const box = { top: old.offsetTop, left: old.offsetLeft, width: old.offsetWidth, height: old.offsetHeight };
    old.classList.add('leaving');
    for (const k in box) old.style[k] = box[k] + 'px';
  }
  items.forEach(i => i.classList.toggle('active', i.dataset.sec === sec));
  page.querySelectorAll('.content .sec').forEach(s => s.classList.toggle('active', s.dataset.sec === sec));
  if (move) {
    const d = to > from ? 1 : -1;
    old.getAnimations({ subtree: true }).forEach(a => a.cancel());
    const out = old.animate([{ opacity: 1, transform: 'none' }, { opacity: 0, transform: `translateY(${-d * 12}px)` }],
      { duration: 120, easing: 'cubic-bezier(.4, 0, 1, 1)', fill: 'forwards' });
    out.onfinish = () => unleave(old);
    el.getAnimations({ subtree: true }).forEach(a => a.cancel());
    [...el.children].slice(0, 10).forEach((c, n) => c.animate(
      [{ opacity: 0, transform: `translateY(${d * 22}px)` }, { opacity: 1, transform: 'none' }],
      { duration: 380, delay: 100 + n * 45, easing: 'cubic-bezier(.22, 1, .36, 1)', fill: 'backwards' }));
  }
  if (sec === 'log') scrollLog(true);
  if (el && el.hasAttribute('data-jump')) Jump.build(el);
  if (typeof Tutorial !== 'undefined') Tutorial.onSection();
}
// 화면 왼쪽 목록 → 그 칸 (모드 화면도 같이 · 나중에 붙어도 되게 위임)
document.addEventListener('click', e => {
  const it = e.target.closest('.page .side .item');
  if (it) selectSection(it.closest('.page'), it.dataset.sec);
});

// ---------------------------------------------------------------- 상태 / 설정
let config = {};
let seq = 0;
const $ = id => document.getElementById(id);
// 같은 글자면 다시 쓰지 않음 — 0.3초마다 받는 상태를 그대로 넣으면 글자 칸이 매번 새로 바뀌어서
// 번역(화면 변화 감지)과 화면 다시 그리기가 쉬지 않고 돎 (이 함수로만 바꾸는 칸에만 씀)
const setText = (el, s) => { if (el && el._txt !== s) { el._txt = s; el.textContent = s; } };
const setData = (el, k, v) => { if (el && el.dataset[k] !== v) el.dataset[k] = v; };

// ---------------------------------------------------------------- 완전 삭제 확인 창
function openConfirm() {
  const c = $('confirm');
  c.hidden = false;
  requestAnimationFrame(() => c.classList.add('on'));
  $('confirmYes').disabled = false;
}
function closeConfirm() {
  const c = $('confirm');
  c.classList.remove('on');
  setTimeout(() => { if (!c.classList.contains('on')) c.hidden = true; }, 200);
}
$('confirmNo').addEventListener('click', closeConfirm);
document.querySelector('#confirm .confirm-dim').addEventListener('click', closeConfirm);
addEventListener('keydown', e => { if (e.key === 'Escape' && !$('confirm').hidden) closeConfirm(); });
$('confirmYes').addEventListener('click', async () => {
  const b = $('confirmYes');
  b.disabled = true;
  try {
    const r = await api('uninstall');
    if (r.error) { toast(r.error); b.disabled = false; return; }
    toast('삭제 중… 창이 곧 닫힙니다');
  } catch { /* 창이 닫히면서 응답이 끊길 수 있음 */ }
});


function toast(msg) {
  const t = $('toast');
  t.textContent = msg;
  t.classList.add('on');
  clearTimeout(toast.h);
  toast.h = setTimeout(() => t.classList.remove('on'), 1800);
}
// 버튼을 눌렀는데 프로그램 응답이 없거나 오류가 나면 조용히 멈추지 않고 알려 줌
addEventListener('unhandledrejection', e => {
  const m = String(e.reason?.message || e.reason || '');
  toast(/fetch|abort|network/i.test(m) ? '프로그램 응답 없음 · 잠시 후 다시 해주세요' : '오류: ' + m.slice(0, 80));
});

function renderSummary() {
  Mods.emit('summary');
  syncMainTiles();
}

let pending = {}, saveTimer = null;
const LOCAL_KEYS = ['play', 'biome', 'pop', 'ret', 'snipe', 'mpop', 'mfish', 'mitem', 'mmerch', 'mcraft', 'mmatch', 'mblood', 'base', 'move'];
function queueSave(patch) {
  Object.assign(pending, patch);
  // 팝핑·바이옴·매크로 탭 설정은 화면 쪽 객체가 원본 (폼이 그 객체를 직접 고치므로 복사본으로 바꾸면 이후 수정이 사라짐)
  for (const [k, v] of Object.entries(patch)) if (!LOCAL_KEYS.includes(k)) config[k] = v;
  renderSummary();
  clearTimeout(saveTimer);
  saveTimer = setTimeout(sendSave, 350);
}
async function sendSave() {
  saveTimer = null;
  const p = pending; pending = {};
  if (!Object.keys(p).length) return;
  try {
    const keep = Object.fromEntries(LOCAL_KEYS.map(k => [k, config[k]]));   // 편집 중인 값은 화면 쪽 객체를 그대로 유지
    config = (await api('set_config', { patch: p }, 15000)).config;
    for (const [k, v] of Object.entries(keep)) if (v) config[k] = v;
    saveFails = 0;
    renderSummary();
  }
  catch {
    // 못 보낸 값은 버리지 않고 다시 보냄 (그 사이에 바뀐 값이 우선) · 알림은 처음 한 번만
    pending = { ...p, ...pending };
    if (!saveFails++) toast('설정 저장 실패 · 다시 시도 중');
    clearTimeout(saveTimer);
    saveTimer = setTimeout(sendSave, Math.min(10000, 1500 * saveFails));
  }
}
let saveFails = 0;
// 기다리는 저장을 지금 바로 보냄 (서버가 방금 바꾼 설정을 써야 할 때)
async function flushSave() {
  if (!saveTimer) return;
  clearTimeout(saveTimer);
  await sendSave();
}

// ---------------------------------------------------------------- 로그
function fmtTime(t) {
  const d = new Date(t * 1000), p = n => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
function scrollLog(force) {
  const l = $('log');
  if (force || l.scrollHeight - l.scrollTop - l.clientHeight < 40) l.scrollTop = l.scrollHeight;
}
// 'n' (알림) = 설정이 비어 있는 등 조건이 안 맞아서 안 한 것 → 로그에 노란색으로 + 알림(토스트)으로도 띄움 (켠 뒤 생긴 것만)
const LOG_SINCE = Date.now() / 1000;
function addLog(x) {
  if (x.color === 'n') {
    if (x.time >= LOG_SINCE - 1) toast(x.msg);
    x = { ...x, color: 'y' };
  }
  const l = $('log');
  const atEnd = l.scrollHeight - l.scrollTop - l.clientHeight < 40;
  const d = document.createElement('div');
  d.innerHTML = `<span class="t"></span><span></span>`;
  d.children[0].textContent = fmtTime(x.time);
  d.children[1].textContent = x.msg;
  if (x.color) d.children[1].className = x.color;
  l.appendChild(d);
  while (l.children.length > 1500) l.firstChild.remove();
  if (atEnd) l.scrollTop = l.scrollHeight;
}
$('clearLog').addEventListener('click', () => { $('log').innerHTML = ''; });

// ---------------------------------------------------------------- 시작 / 중지 (메인 화면 기능별 버튼)
// 바이옴 매크로 = 바이옴 매크로 설정의 켜기 · 매크로 = 매크로 탭 기능 전체 · 모드가 타일을 더 붙일 수 있음 (예: 오토 스나이핑)
const tile = k => document.querySelector(`.main-tile[data-main="${k}"]`);
function setTile(k, on, btnText) {
  const t = tile(k);
  if (t.classList.contains('on') !== !!on) t.classList.toggle('on', !!on);
  setText(t.querySelector('.mt-go'), btnText || (on ? '끄기' : '시작'));
}
$('mgBiome').addEventListener('click', () => setBioEnabled(!bio().enabled));
let macroClickAt = 0;                        // 화면에서 누른 직후엔 서버 값으로 되돌리지 않음 (저장이 늦게 갈 수 있음)
$('mgMacro').addEventListener('click', () => {
  const on = !config.macro_on;
  config.macro_on = on; macroClickAt = Date.now();
  queueSave({ macro_on: on });
  if (!on) { api('mpop_stop'); api('mfish_stop'); }
  syncMainTiles();
  toast('매크로 ' + (on ? '켜짐' : '꺼짐'));
});
// 메인 타일 이름 → 그 설정 탭 (모드 타일도 같이 · 나중에 붙어도 되게 위임)
document.addEventListener('click', e => { const b = e.target.closest('[data-tab-go]'); if (b) setTab(b.dataset.tabGo, true); });
// 다른 화면 열기 버튼 (data-open-page="키") — 다른 탭의 화면이면 그 탭으로 넘긴 뒤
document.addEventListener('click', e => {
  const b = e.target.closest('[data-open-page]');
  if (!b) return;
  const key = b.dataset.openPage, m = MENU.find(x => x.key === key);
  const go = () => openPage(key);
  if (current) { closePage().then(() => { if (m && curTab !== m.tab) { setTab(m.tab); setTimeout(go, 380); } else go(); }); }
  else if (m && curTab !== m.tab) { setTab(m.tab); setTimeout(go, 380); } else go();
});

// 켜진 매크로 탭 기능 수
const macroFeatures = () => FEATS.filter(k => featCfg(k).enabled).length;
let lastBio = null, lastMpop = null, lastMfish = null, lastMitem = null, lastMmerch = null, lastMcraft = null, lastMmatch = null, lastMblood = null;
function syncMainTiles() {
  const bOn = !!bio().enabled;
  setTile('biome', bOn);
  const cur = lastBio && lastBio.current;
  setText($('mtBiome'), bOn ? `작동 중 · ${cur ? bioName(cur) : '바이옴 확인 중'}`
    : (bio().player ? '꺼짐' : '꺼짐 · 플레이어 이름 필요'));
  const mOn = !!config.macro_on, n = macroFeatures();
  const sniping = !!(lastBio && lastBio.muted);
  setTile('macro', mOn, mOn ? '중지 · F3' : '시작 · F3');
  if (tile('macro').classList.contains('wait') !== (mOn && sniping)) tile('macro').classList.toggle('wait', mOn && sniping);
  const mText = !mOn ? (n ? `꺼짐 · 기능 ${n}개 켜짐` : '꺼짐')
    : lastMpop && lastMpop.running ? (lastMpop.msg || '포션 사용 중')
    : lastMblood && lastMblood.running ? (lastMblood.msg || '블러드 레인 만드는 중')
    : lastMitem && lastMitem.running ? (lastMitem.msg || '아이템 사용 중')
    : lastMmerch && lastMmerch.running && lastMmerch.job === 'buy' ? (lastMmerch.msg || '상인 구매 중')
    : lastMcraft && lastMcraft.running ? (lastMcraft.msg || '포션 제작 중')
    : lastMmatch && lastMmatch.running ? (lastMmatch.msg || '메모리 매치 중')
    : lastMfish && lastMfish.running ? (lastMfish.msg || '낚시 중')
    : sniping ? '스나이핑 중이라 대기'
    : n ? `작동 중 · 기능 ${n}개` : '켜진 기능 없음';
  setText($('mtMacro'), mText);
  // 매크로 설정 화면 위쪽: 매크로 버튼 상태
  setData($('macroDot'), 's', mOn ? (sniping ? 'waiting' : 'flux') : '');
  setText($('macroState'), mOn ? `매크로 켜짐 · ${mText}` : (n ? `매크로 꺼짐 · 기능 ${n}개 켜짐` : '매크로 꺼짐'));
  const on = [bOn && '바이옴 매크로', mOn && '매크로', ...Mods.emit('active')].filter(Boolean);
  setText($('modeText'), on.length ? `작동 중: ${on.join(' · ')}` : '모두 꺼짐 — 아래 버튼으로 켜기');
  syncFeatItems();
  Mods.emit('tiles');
}

function setStatus([state, text]) {
  document.querySelectorAll('[data-status-dot]').forEach(d => setData(d, 's', state));
  document.querySelectorAll('[data-status-text]').forEach(t => setText(t, text));
}

// ---------------------------------------------------------------- 주기적으로 엔진 상태 받기
let pollFails = 0;
async function poll() {
  try {
    const r = await api('poll', { since: seq }, 8000);
    pollFails = 0;
    seq = r.seq;
    setStatus(r.status);
    r.logs.forEach(addLog);
    updateCrash(r.crash);
    updatePlay(r.play, r.roblox, r.pop);
    updateRet(r.ret);
    updateBiome(r.biome);
    updateMpop(r.mpop);
    updateMfish(r.mfish);
    updateMove(r.move);
    lastBio = r.biome; lastMpop = r.mpop; lastMfish = r.mfish; lastMitem = r.mitem; lastMmerch = r.mmerch; lastMcraft = r.mcraft; lastMmatch = r.mmatch;
    lastMblood = r.mblood;
    updateMitem(r.mitem);
    updateMblood(r.mblood);
    updateMcraft(r.mcraft);
    updateMmatch(r.mmatch);
    updateMmerch(r.mmerch);
    updateOnline(r.online);
    lastStats = r.stats || null;
    if (current === 'mstats') renderStats();
    const row = $('acOnline').closest('.row');
    if (row.hidden !== !r.online_on) row.hidden = !r.online_on;   // 사용자 수 서버가 아직 없으면 스위치도 숨김
    if (r.macro_on !== undefined && !!r.macro_on !== !!config.macro_on && Date.now() - macroClickAt > 2000)
      config.macro_on = r.macro_on;            // F3 · F7 로 켜고 끈 것
    Mods.emit('poll', r);
    syncMainTiles();
  } catch (err) {
    // 한두 번 늦는 건 넘김 (매크로가 OCR · 클릭 중이면 응답이 잠깐 늦을 수 있음)
    if (++pollFails >= 3) setStatus(['error', '프로그램과 연결 끊김']);
    if (pollFails === 3) console.warn('poll', err);
  }
  // 창이 최소화돼 있거나 · 다른 창(게임)을 보고 있거나 · Play 클릭 중이면 덜 자주 받아옴 (그동안 CPU 를 게임 · 매크로에 양보)
  // 연결이 끊겼으면 천천히 다시 시도
  setTimeout(poll, pollFails >= 3 ? 2000 : document.hidden ? 3000 : (playBusy ? 1000 : (document.hasFocus() ? 300 : 1000)));
}

// ---------------------------------------------------------------- 오토 팝핑 (Play 버튼)
const play = () => (config.play ||= { pos: null, skip_pos: null, interval: 0.15, load_wait: 5, max_time: 300 });
const savePlay = () => queueSave({ play: JSON.parse(JSON.stringify(play())) });
const pct = v => (v == null ? '-' : Math.round(v * 1000) / 10 + '%');
const PICKS = { play: { key: 'pos', code: 'playPos', btn: 'playPick', name: 'Play 버튼' },
                skip: { key: 'skip_pos', code: 'skipPos', btn: 'skipPick', name: 'Click to skip 버튼' } };

function showPlayPos() {
  for (const d of Object.values(PICKS)) {
    const p = play()[d.key];
    $(d.code).textContent = p ? `${pct(p[0])}, ${pct(p[1])}` : '지정 안 됨';
    $(d.code).classList.toggle('unset', !p);
  }
}
function fillPlay() {
  const p = play();
  if (!$('playTplRow')) {
    const w = document.createElement('div');
    w.innerHTML = tplRowHTML();
    w.firstElementChild.id = 'playTplRow';
    $('playPos').closest('.card').prepend(w.firstElementChild);
  }
  $('playWait').value = p.load_wait;
  $('playInterval').value = p.interval;
  $('playMax').value = p.max_time;
  showPlayPos();
}
for (const [id, key, min] of [['playWait', 'load_wait', 0], ['playInterval', 'interval', 0.05], ['playMax', 'max_time', 10]]) {
  $(id).addEventListener('input', e => {
    const n = parseFloat(e.target.value);
    if (Number.isFinite(n) && n >= min) { play()[key] = n; savePlay(); }
  });
}
let picking = false;
for (const [which, d] of Object.entries(PICKS)) {
  $(d.btn).addEventListener('click', async () => {
    if (picking) { api('pick_cancel'); return; }
    picking = true;
    const b = $(d.btn);
    b.textContent = `로블록스 화면에서 ${d.name} 클릭`; b.classList.add('waiting');
    try {
      const r = await api('play_pos', { which });
      if (r.error) toast(r.error);
      else { play()[d.key] = r.pos; showPlayPos(); savePlay(); toast(`${d.name} 위치 저장`); Tutorial.refresh(); }
    } finally {
      picking = false; b.textContent = '위치 지정'; b.classList.remove('waiting');
    }
  });
}
$('playTest').addEventListener('click', async () => {
  const r = await api('play_test');
  toast(r.error || '1초 후 Play / Click to skip 번갈아 클릭 시작 · 게임 입장이 확인되면 자동 정지');
});
$('playStop').addEventListener('click', () => api('play_stop'));

let playBusy = false;
// 로블록스가 안 켜져 있으면 왼쪽 아래 알림 → 누르면 내 브섭 링크로 접속
// 사라지는 경우: 로블록스가 감지되거나 ✕ (✕ 로 닫으면 로블록스가 한 번 감지된 뒤 다시 꺼질 때까지 안 뜸)
let rbxDismissed = false, rbxSeen = null;
// 매크로 작동 중 로블록스가 갑자기 꺼짐 → 남은 시간 표시, [대기] 누르면 복귀 취소 (그 뒤엔 일반 '감지 안 됨' 알림)
let crashShown = false;
function updateCrash(crash) {
  const n = $('rbxCrash');
  if (crash) setText($('crashText'), `${Math.ceil(crash.left)}초 후 매크로 복귀를 실행합니다`);
  const show = !!crash;
  if (show === crashShown) return;
  crashShown = show;
  if (show) { n.hidden = false; requestAnimationFrame(() => n.classList.add('on')); }
  else { n.classList.remove('on'); setTimeout(() => { if (!crashShown) n.hidden = true; }, 300); }
}
$('crashWait').addEventListener('click', async () => {
  updateCrash(null);
  rbxDismissed = false;
  try { await api('crash_cancel'); } catch {}
  toast('복귀 취소 — 대기');
});

function updateRbxNotice(roblox) {
  const n = $('rbxNotice');
  if (roblox) { rbxDismissed = false; $('rbxJoin').disabled = false; $('rbxJoin').textContent = "Sol's RNG에 접속하시겠습니까?"; }
  const show = !roblox && !rbxDismissed && !crashShown;   // 갑자기 꺼짐 알림이 떠 있으면 그것만
  if (show === rbxSeen) return;
  rbxSeen = show;
  if (show) { n.hidden = false; requestAnimationFrame(() => n.classList.add('on')); }
  else { n.classList.remove('on'); setTimeout(() => { if (!rbxSeen) n.hidden = true; }, 300); }
}
$('rbxClose').addEventListener('click', () => { rbxDismissed = true; updateRbxNotice(false); });
$('rbxJoin').addEventListener('click', async () => {
  const r = await api('ret_join');
  if (r.error) return toast(r.error);
  $('rbxJoin').disabled = true; $('rbxJoin').textContent = '접속 중… (로블록스가 뜨면 사라짐)';
  toast('내 서버로 접속');
});

// 복구 · 게임 입장 위쪽: 게임 입장(Play 클릭) 상태 (모드 화면에도 [data-play-dot]) · 오토 팝핑 설정 위쪽 (모드): 오토 팝핑 상태
function updatePlay(st, roblox, pst) {
  updateRbxNotice(roblox);
  playBusy = !!(st && st.running) || !!(pst && pst.running);
  setText($('robloxState'), roblox ? '로블록스 창 감지됨' : '로블록스 창 없음');
  if ($('robloxState').classList.contains('ok') !== !!roblox) $('robloxState').classList.toggle('ok', !!roblox);
  const running = !!(st && st.running);
  document.querySelectorAll('[data-play-dot]').forEach(d => setData(d, 's', running ? 'flux' : ''));
  document.querySelectorAll('[data-play-state]').forEach(t => setText(t, running ? (st.msg || '실행 중') : '대기'));
  $('playTest').disabled = running;
  const popping = !!(pst && pst.running);
  setData($('popDot'), 's', popping ? 'flux' : '');
  setText($('popState'), popping ? (pst.msg || '오토 팝핑 중') : '대기');
  document.querySelectorAll('[data-bio-test^="pop:"]').forEach(b => { b.disabled = popping; });
}

// ---------------------------------------------------------------- 오토 팝핑 (레어 바이옴 포션)
const POP_POS = [['inventory_pos', 'Inventory 버튼'], ['items_pos', 'Items 버튼'], ['search_pos', 'Search 버튼'],
                 ['item_pos', '아이템 칸', '검색 결과 첫 칸'], ['amount_pos', '수량 입력칸'], ['use_pos', 'Use 버튼']];
const POP_DELAYS = [['after_join', 'Play 후 게임 입장 → 시작', 7.5], ['inventory', 'Inventory 클릭 후', 0.5],
  ['items', 'Items 클릭 후', 0.5], ['search', 'Search 클릭 후', 0.5], ['typing', '아이템 이름 입력 + 엔터 후', 0.5],
  ['item_click', '아이템 클릭 후', 0.35], ['dbl_gap', '수량칸 더블클릭 간격', 0.1], ['after_dbl', '수량칸 더블클릭 후', 0.5],
  ['after_enter', '개수 입력 + 엔터 후', 0.35]];
const POP_BIOMES = ['CYBERSPACE', 'GLITCHED', 'DREAMSPACE'];
// 일반 스나이핑 바이옴 (레어 바이옴 말고 Windy · Snowy · Rainy 빼고 전부): 들어가서 바이옴이 끝날 때까지 머묾 · 팝핑은 골라서 (기본 꺼짐)
// 엔진(biome.STAY_BIOMES)과 같은 순서
const STAY_BIOMES = ['RADIANTFOREST', 'SINGULARITY', 'LOSTFOREST', 'BLOODRAIN', 'PUMPKINMOON', 'GRAVEYARD',
                     'NULL', 'CORRUPTION', 'HEAVEN', 'STARFALL', 'HELL', 'SANDSTORM'];
const SNIPE_BIOMES = [...POP_BIOMES, ...STAY_BIOMES];
// 바이옴 키 → 보여줄 이름 (여러 낱말 바이옴은 키에 띄어쓰기가 없음)
const BIOME_NAME = { LOSTFOREST: 'LOST FOREST', RADIANTFOREST: 'RADIANT FOREST', PUMPKINMOON: 'PUMPKIN MOON', BLOODRAIN: 'BLOOD RAIN' };
const bioName = k => BIOME_NAME[k] || k;
const pop = () => (config.pop ||= {});
const savePop = () => queueSave({ pop: JSON.parse(JSON.stringify(pop())) });
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const fmtPos = p => p ? `${pct(p[0])}, ${pct(p[1])}` : '지정 안 됨';
const fmtReg = r => r ? `${pct(r[0])}, ${pct(r[1])} → ${pct(r[2])}, ${pct(r[3])}` : '지정 안 됨';

async function pickWith(btn, label, call) {
  if (picking) { api('pick_cancel'); return null; }   // 지정 중에 다시 누르면 취소 (창이 안 떠서 멈춘 경우에도 풀림)
  picking = true;
  const old = I18N.source(btn);
  btn.textContent = label; btn.classList.add('waiting');
  try {
    const r = await call();
    if (r.error) { toast(r.error); return null; }
    return r;
  } finally {
    picking = false; btn.textContent = old; btn.classList.remove('waiting');
  }
}

// ---------------------------------------------------------------- 위치 템플릿 (로블록스 창 비율별 기본 위치)
// 값은 로블록스 창 기준 비율 — Play · Click to skip · 오토 팝핑 버튼 6개 · OCR 영역을 한 번에 채움
const POS_TEMPLATES = {
  '16:9': { pos: [0.139, 0.925], skip_pos: [0.5, 0.752],
            inventory_pos: [0.018, 0.474], items_pos: [0.663, 0.312], search_pos: [0.458, 0.34],
            item_pos: [0.443, 0.44], amount_pos: [0.296, 0.534], use_pos: [0.356, 0.535],
            ocr_region: [0.418, 0.399, 0.466, 0.484] },
};
const tplRowHTML = () => `
  <div class="row tpl-pick"><span>위치 템플릿<small>화면 비율에 맞는 기본 위치를 한 번에 채움</small></span>
    <span class="pos"><select data-pos-tpl>${Object.keys(POS_TEMPLATES).map(k => `<option value="${k}">${k}</option>`).join('')}</select>
    <button class="btn mini" type="button" data-pos-apply>적용</button></span></div>`;
function applyPosTemplate(name) {
  const t = POS_TEMPLATES[name];
  if (!t) return;
  const p = play(), q = pop();
  p.pos = [...t.pos]; p.skip_pos = [...t.skip_pos];
  for (const [k] of POP_POS) q[k] = [...t[k]];
  q.ocr_region = [...t.ocr_region];
  showPlayPos(); renderPopSet(); savePlay(); savePop();
  toast(`${name} 템플릿 적용`);
  Tutorial.refresh();
}
// 이미 지정한 위치가 있으면 한 번 더 눌러야 덮어씀 (실수로 날리지 않게)
document.addEventListener('click', e => {
  const b = e.target.closest('[data-pos-apply]');
  if (!b) return;
  const name = b.parentElement.querySelector('[data-pos-tpl]').value;
  const t = POS_TEMPLATES[name];
  const same = (a, c) => JSON.stringify(a || null) === JSON.stringify(c);
  const custom = [['pos', play()], ['skip_pos', play()], ...POP_POS.map(([k]) => [k, pop()]), ['ocr_region', pop()]]
    .some(([k, o]) => o[k] && !same(o[k], t[k]));
  if (custom && !(b._armed > Date.now())) {
    b._armed = Date.now() + 3000;
    b.textContent = '한 번 더 누르면 덮어쓰기';
    clearTimeout(b._t);
    b._t = setTimeout(() => { b.textContent = '적용'; b._armed = 0; }, 3000);
    return;
  }
  clearTimeout(b._t); b._armed = 0;
  document.querySelectorAll('[data-pos-apply]').forEach(x => { x.textContent = '적용'; });
  document.querySelectorAll('[data-pos-tpl]').forEach(x => { x.value = name; });
  applyPosTemplate(name);
});

// 스나이프 오토 팝핑이 실제로 쓰는 위치 (공통 위치와 연동이면 공통 위치)
const popPos = () => {
  const p = pop(), b = base(), o = { ...p };
  const [first, other] = p.use_base ? [b, p] : [p, b];
  for (const k of POP_POS_KEYS) o[k] = first[k] || other[k];   // 고른 쪽에 비어 있으면 다른 쪽 값 (서버와 같게)
  return o;
};
// 오토 스나이핑을 시작하는 데 필요한데 비어 있는 것 (서버 pop_missing 과 같게: Play · Click to skip + 인벤토리 버튼 6개 + OCR 영역)
const playMissing = () => !config.play?.pos || !config.play?.skip_pos;
const popPosMissing = () => !popPos().ocr_region || POP_POS.some(([k]) => !popPos()[k]);
const popMissing = () => playMissing() || popPosMissing();
function renderPopSet() {
  if (!$('popSetForm')) return;                   // 오토 팝핑 화면은 스나이핑 모드에 있음
  const box = $('popSetForm');
  const p = pop(), linked = !!p.use_base, e = popPos();
  const pick = (attr, k, label) => linked ? '' : `<button class="btn mini ghost" type="button" ${attr}${k ? `="${k}"` : ''}>${label}</button>`;
  box.innerHTML = `
    <label class="row"><span>매크로 공통 위치와 연동<small>켜면 매크로 설정 → 공통 위치의 인벤토리 위치 · OCR 영역을 같이 씀</small></span>
      <span class="switch"><input type="checkbox" id="popLink" ${linked ? 'checked' : ''}><i></i></span></label>` +
    (linked ? '' : tplRowHTML()) + POP_POS.map(([k, name, sub]) => `
    <div class="row"><span>${name} 위치<small>${sub ? sub + ' · ' : ''}${linked ? '공통 위치와 같음' : '직접 지정 필요'}</small></span>
      <span class="pos"><code class="${e[k] ? '' : 'unset'}" data-code="${k}">${fmtPos(e[k])}</code>
      ${pick('data-pick', k, '위치 지정')}</span></div>`).join('') + `
    <div class="row"><span>OCR 영역<small>검색 결과 아이템 이름·개수 (예: Warp Potion x23)${linked ? ' · 공통 위치와 같음' : ''}</small></span>
      <span class="pos"><code class="${e.ocr_region ? '' : 'unset'}" id="popRegion">${fmtReg(e.ocr_region)}</code>
      ${pick('id="popRegionPick"', '', '드래그로 지정')}
      <button class="btn mini ghost" type="button" id="popOcrTest">OCR 테스트</button></span></div>
    <label class="row"><span>이름 일치율 기준<small>이 이상 같아야 사용 · 아니면 다시 검색 후 건너뜀 (%)</small></span>
      <input type="number" min="1" max="100" step="5" id="popThreshold" value="${p.match_threshold ?? 70}"></label>`;
  $('popLink').addEventListener('change', ev => setPopLink(ev.target.checked));
  box.querySelectorAll('[data-pick]').forEach(b => b.addEventListener('click', async () => {
    const k = b.dataset.pick, name = POP_POS.find(x => x[0] === k)[1];
    const r = await pickWith(b, `로블록스 화면에서 ${name} 클릭`, () => api('pop_pos', { key: k }));
    if (r) { pop()[k] = r.pos; renderPopSet(); savePop(); toast(`${name} 위치 저장`); Tutorial.refresh(); }
  }));
  $('popRegionPick')?.addEventListener('click', async e => {
    const r = await pickWith(e.currentTarget, '로블록스 화면에서 드래그', () => api('pop_region'));
    if (r) { pop().ocr_region = r.region; renderPopSet(); savePop(); toast('OCR 영역 저장'); Tutorial.refresh(); }
  });
  $('popOcrTest').addEventListener('click', async e => {
    const b = e.currentTarget;
    b.disabled = true; b.textContent = '읽는 중…';
    try {
      const r = await Promise.race([api('pop_ocr_test', {}), wait(25000).then(() => ({ error: 'OCR 응답 없음 · 로그 확인' }))]);
      if (r.error) toast(r.error);
      else toast((r.text ? `이름: ${r.name || '-'} · 개수: ${r.count ?? '표시 없음(1개)'}` : '읽은 글자 없음') + ` · ${r.engine}`);
    } catch (err) { toast('OCR 오류: ' + err.message); }
    finally { b.disabled = false; b.textContent = 'OCR 테스트'; }
  });
  $('popThreshold').addEventListener('input', e => {
    const n = parseFloat(e.target.value);
    if (n >= 1 && n <= 100) { pop().match_threshold = n; savePop(); }
  });
}

function renderPopDelays() {
  if (!$('popDelayForm')) return;
  const d = (pop().delays ||= {});
  const box = $('popDelayForm');
  box.innerHTML = POP_DELAYS.map(([k, name, def]) => `
    <label class="row"><span>${name}<small>기본 ${def}초</small></span>
      <input type="number" min="0" step="0.05" data-delay="${k}" value="${d[k] ?? def}"></label>`).join('');
  box.querySelectorAll('[data-delay]').forEach(i => i.addEventListener('input', () => {
    const n = parseFloat(i.value);
    if (Number.isFinite(n) && n >= 0) { d[i.dataset.delay] = n; savePop(); }
  }));
}

// src: 설정 묶음 (오토 팝핑 pop / 매크로 탭 mpop) · save: 저장 · attr: 템플릿 칸 표시
function renderTemplate(b, src = pop, save = savePop, attr = 'data-tpl') {
  const tpls = (src().templates ||= {});
  const t = (tpls[b] ||= { mode: 'all', items: [] });
  const box = document.querySelector(`.tpl[${attr}="${b}"]`);
  if (!box) return;
  const again = () => renderTemplate(b, src, save, attr);
  box.innerHTML = `
    <label class="row"><span>사용 방식<small>목록 위쪽일수록 먼저 · 목록이 비어 있으면 이 바이옴에선 오토 팝핑 안 함</small></span>
      <select data-mode><option value="all">목록 전부 순서대로</option><option value="one">위에서부터 조건 맞는 하나만</option></select></label>
    <div class="tpl-list"></div>
    <div class="tpl-foot"><button class="btn ghost mini" type="button" data-add>+ 포션 추가</button>
      <span class="hint">최소 보유: OCR 로 읽은 개수가 이보다 적으면 그 포션은 건너뜀</span></div>`;
  const sel = box.querySelector('[data-mode]');
  sel.value = t.mode === 'one' ? 'one' : 'all';
  sel.addEventListener('change', () => { t.mode = sel.value; save(); });
  const list = box.querySelector('.tpl-list');
  if (!t.items.length) list.innerHTML = '<div class="empty">포션 없음 · 아래 버튼으로 추가</div>';
  t.items.forEach((it, i) => {
    const all = String(it.amount).toUpperCase() === 'ALL';
    const row = document.createElement('div');
    row.className = 'tpl-row';
    row.innerHTML = `<span class="num">${i + 1}</span>
      <input class="tpl-name" placeholder="포션 이름 (예: Warp Potion)" spellcheck="false" value="${esc(it.name || '')}">
      <select class="tpl-amt-mode"><option value="all">전부</option><option value="n">직접</option></select>
      <input class="tpl-amt" type="number" min="1" step="1" value="${all ? 1 : it.amount}" ${all ? 'disabled' : ''} title="사용 개수">
      <label class="tpl-min" title="최소 보유">최소 <input type="number" min="1" step="1" value="${it.min_have || 1}"></label>
      <button class="ib" data-a="up" title="위로">↑</button><button class="ib" data-a="down" title="아래로">↓</button>
      <button class="ib del" data-a="del" title="삭제">${icon('x')}</button>`;
    const [nameI, modeS, amtI, minI] = [row.querySelector('.tpl-name'), row.querySelector('.tpl-amt-mode'),
      row.querySelector('.tpl-amt'), row.querySelector('.tpl-min input')];
    modeS.value = all ? 'all' : 'n';
    nameI.addEventListener('input', () => { it.name = nameI.value; save(); });
    modeS.addEventListener('change', () => {
      it.amount = modeS.value === 'all' ? 'ALL' : Math.max(1, parseInt(amtI.value, 10) || 1);
      amtI.disabled = modeS.value === 'all'; save();
    });
    amtI.addEventListener('input', () => { const n = parseInt(amtI.value, 10); if (n >= 1) { it.amount = n; save(); } });
    minI.addEventListener('input', () => { const n = parseInt(minI.value, 10); if (n >= 1) { it.min_have = n; save(); } });
    row.querySelectorAll('.ib').forEach(btn => btn.addEventListener('click', () => {
      const a = btn.dataset.a, L = t.items;
      if (a === 'up' && i > 0) [L[i - 1], L[i]] = [L[i], L[i - 1]];
      else if (a === 'down' && i < L.length - 1) [L[i + 1], L[i]] = [L[i], L[i + 1]];
      else if (a === 'del') L.splice(i, 1);
      else return;
      save(); again(); Tutorial.refresh();
    }));
    list.appendChild(row);
  });
  box.querySelector('[data-add]').addEventListener('click', () => {
    t.items.push({ name: '', amount: 'ALL', min_have: 1 });
    save(); again(); Tutorial.refresh();
    box.querySelector('.tpl-row:last-child .tpl-name')?.focus();
  });
}

// ---------------------------------------------------------------- 바이옴별 팝핑 칸 (켜기 + 포션 목록 · 이름을 누르면 펼침)
// 오토 팝핑(pop · 스나이프 탭)과 레어 바이옴 자동 팝핑(mpop · 매크로 탭)이 같이 씀
const BIO_LABEL = { CYBERSPACE: 'Cyberspace', GLITCHED: 'Glitched', DREAMSPACE: 'Dreamspace',
                    RADIANTFOREST: 'Radiant Forest', SINGULARITY: 'Singularity', LOSTFOREST: 'Lost Forest',
                    BLOODRAIN: 'Blood Rain', PUMPKINMOON: 'Pumpkin Moon', GRAVEYARD: 'Graveyard', NULL: 'Null',
                    CORRUPTION: 'Corruption', HEAVEN: 'Heaven', STARFALL: 'Starfall', HELL: 'Hell', SANDSTORM: 'Sandstorm' };
// 일반 스나이핑 바이옴은 기본 꺼짐 (꺼져 있으면 들어가서 팝핑 없이 바이옴이 끝날 때까지 머묾) · 레어 바이옴은 기본 켜짐
const popOn = b => { const on = pop().biomes_on || {}; return STAY_BIOMES.includes(b) ? on[b] === true : on[b] !== false; };
const mpopOn = b => (mpop().biomes_on || {})[b] !== false;
const BIO_KIND = {
  pop: { src: () => pop(), save: () => savePop(), attr: 'data-tpl', on: 'data-pop-on', isOn: popOn, def: 'pop_default', test: 'pop_test',
         testMsg: '오토 팝핑 테스트 시작 · 인벤토리 열기부터 · 정지: 위쪽 [정지] 또는 F7' },
  mpop: { src: () => mpop(), save: () => saveMpop(), attr: 'data-mtpl', on: 'data-mpop-on', isOn: mpopOn, def: 'mpop_default', test: 'mpop_test',
          testMsg: '레어 바이옴 자동 팝핑 테스트 시작 · 인벤토리 열기부터 · 정지: 위쪽 [정지] 또는 F7' },
};
const bioOpen = new Set();          // 펼쳐 둔 칸 ('pop:GLITCHED')
function bioCardHTML(kind, b) {
  const K = BIO_KIND[kind], id = `${kind}:${b}`, open = bioOpen.has(id);
  return `
    <div class="bio-card${open ? ' open' : ''}${RARE.includes(b) ? ' rare' : ''}" data-bio-card="${id}">
      <div class="bio-top">
        <button class="bio-open" type="button" data-bio-toggle="${id}" aria-expanded="${open}"><span class="caret">${icon('chev')}</span>
          <b>${BIO_LABEL[b]}</b><span class="bio-sum" data-bio-sum="${id}"></span></button>
        <label class="switch"><input type="checkbox" ${K.on}="${b}" ${K.isOn(b) ? 'checked' : ''}><i></i></label>
      </div>
      <div class="bio-body"><div class="bio-in">
        <div class="form tpl" ${K.attr}="${b}"></div>
        <div class="bio-acts"><button class="btn ghost mini" type="button" data-bio-default="${id}">기본 템플릿으로</button>
          <button class="btn mini" type="button" data-bio-test="${id}">이 템플릿 테스트</button></div>
      </div></div>
    </div>`;
}
// 접힌 칸에 보이는 요약: 켜짐/꺼짐 · 포션 개수 · 사용 방식
function syncBioSum(kind, b) {
  const K = BIO_KIND[kind], el = document.querySelector(`[data-bio-sum="${kind}:${b}"]`);
  if (!el) return;
  const t = (K.src().templates || {})[b] || {}, n = (t.items || []).filter(it => (it.name || '').trim()).length;
  const stay = kind === 'pop' && STAY_BIOMES.includes(b);
  let text, warn = false;
  if (!K.isOn(b)) text = stay ? '꺼짐 · 팝핑 없이 바이옴이 끝날 때까지 머묾' : kind === 'pop' ? '꺼짐 · 들어가서 바로 복귀' : '꺼짐';
  else if (!n) { text = '포션 목록이 비어 있음 — 팝핑 안 함'; warn = true; }
  else text = `포션 ${n}개 · ${t.mode === 'one' ? '조건 맞는 하나만' : '전부 순서대로'}`;
  el.textContent = text;
  el.classList.toggle('warn', warn);
  el.closest('.bio-card')?.classList.toggle('off', !K.isOn(b));
}
function renderBioList(kind) {
  const K = BIO_KIND[kind];
  const lists = kind === 'pop' ? [['popBioRare', POP_BIOMES], ['popBioStay', STAY_BIOMES]] : [['mpopBio', POP_BIOMES]];
  for (const [id, bs] of lists) {
    if (!$(id)) continue;                        // 스나이핑 모드가 없으면 오토 팝핑 바이옴 칸도 없음
    $(id).innerHTML = bs.map(b => bioCardHTML(kind, b)).join('');
    bs.forEach(b => {
      renderTemplate(b, K.src, () => { K.save(); syncBioSum(kind, b); }, K.attr);
      syncBioSum(kind, b);
    });
  }
}
// 켜기 스위치: 같은 바이옴 스위치(바이옴별 칸 · 메인 화면 ▾)는 전부 같은 설정
function setBioOn(kind, b, v) {
  const K = BIO_KIND[kind];
  (K.src().biomes_on ||= {})[b] = !!v;
  K.save();
  document.querySelectorAll(`[${K.on}="${b}"]`).forEach(i => { i.checked = !!v; });
  syncBioSum(kind, b);
  Quick.sync();
}
document.addEventListener('change', e => {
  const t = e.target;
  if (t.dataset.popOn) setBioOn('pop', t.dataset.popOn, t.checked);
  else if (t.dataset.mpopOn) setBioOn('mpop', t.dataset.mpopOn, t.checked);
});
document.addEventListener('click', async e => {
  const tg = e.target.closest('[data-bio-toggle]');
  if (tg) {
    const id = tg.dataset.bioToggle, card = tg.closest('.bio-card');
    const open = !card.classList.contains('open');
    card.classList.toggle('open', open);
    tg.setAttribute('aria-expanded', open);
    open ? bioOpen.add(id) : bioOpen.delete(id);
    setTimeout(() => Tutorial.refresh(), 280);       // 펼쳐지는 동안 강조 박스가 따라가도록
    return;
  }
  const df = e.target.closest('[data-bio-default]'), ts = e.target.closest('[data-bio-test]');
  if (df) {
    const [kind, b] = df.dataset.bioDefault.split(':'), K = BIO_KIND[kind];
    const r = await api(K.def, { biome: b });
    if (r.error) return toast(r.error);
    (K.src().templates ||= {})[b] = r.template;
    renderTemplate(b, K.src, () => { K.save(); syncBioSum(kind, b); }, K.attr);
    syncBioSum(kind, b); Tutorial.refresh();
    toast('기본 템플릿으로 되돌림');
  } else if (ts) {
    const [kind, b] = ts.dataset.bioTest.split(':'), K = BIO_KIND[kind];
    await flushSave();                                // 방금 고친 포션 목록으로 테스트
    const r = await api(K.test, { biome: b });
    toast(r.error || K.testMsg);
  }
});

function fillPop() {
  renderPopSet();
  renderBioList('pop');
  renderPopDelays();
}

// ---------------------------------------------------------------- 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버)
// 포션 목록 · 켜진 바이옴은 따로(mpop), 버튼 위치 · OCR 영역은 공통 위치(base), 딜레이 · 일치율은 오토 팝핑(pop) 설정을 같이 씀
const mpop = () => (config.mpop ||= {});
const base = () => (config.base ||= {});         // 공통 위치 (인벤토리 위치 · OCR 영역 · 알림 영역 …)
const saveMpop = () => queueSave({ mpop: JSON.parse(JSON.stringify(mpop())) });
const POP_POS_KEYS = ['inventory_pos', 'items_pos', 'search_pos', 'item_pos', 'amount_pos', 'use_pos', 'ocr_region'];
// 기능 칸 맨 위: 켜기 · 준비 상태 (필요한 위치 · 장소가 다 지정됐는지)
const onRow = (k, sub) => `
    <label class="row on-row"><span>켜기<small>${sub}</small></span>
      <span class="switch"><input type="checkbox" id="${k}On" data-feat="${k}" ${featCfg(k).enabled ? 'checked' : ''}><i></i></span></label>`;
const readyRow = k => `
    <div class="row"><span>준비 상태<small>필요한 위치 · 장소가 다 지정됐는지 · 비어 있으면 위쪽 [설정 안내]로 하나씩</small></span>
      <span class="ready" data-ready="${k}"></span></div>`;
function renderMpop() {
  const m = mpop();
  $('mpopForm').innerHTML = onRow('mpop', '레어 바이옴이 감지되면 아래 바이옴별 포션 목록대로 사용') + readyRow('mpop');
  $('mpopSet').innerHTML = `
    <label class="row"><span>시작 전 대기<small>바이옴 감지 후 인벤토리를 열기까지 (초) · 기본 1</small></span>
      <input type="number" min="0" max="60" step="0.5" id="mpopDelay" value="${m.start_delay ?? 1}"></label>
    <label class="row"><span>다 쓰고 인벤토리 닫기<small>Inventory 버튼을 한 번 더 눌러 닫음</small></span>
      <span class="switch"><input type="checkbox" id="mpopClose" ${m.close_inventory !== false ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>버튼 위치 · OCR 영역<small>공통 위치 → 인벤토리 를 씀</small></span><span>공통 위치와 같음</span></div>
    <div class="row"><span>딜레이 · 이름 일치율<small>오토 팝핑 설정(스나이프 탭)에 지정한 것을 같이 씀</small></span><span>오토 팝핑과 같음</span></div>`;
  $('mpopDelay').addEventListener('input', e => {
    const n = parseFloat(e.target.value);
    if (n >= 0 && n <= 60) { m.start_delay = n; saveMpop(); }
  });
  $('mpopClose').addEventListener('change', e => { m.close_inventory = e.target.checked; saveMpop(); });
  syncReady('mpop');
}
function fillMpop() {
  renderMpop();
  renderBioList('mpop');
}
function updateMpop(st) {
  const running = !!(st && st.running);
  setData($('mpopDot'), 's', running ? 'flux' : '');
  setText($('mpopState'), running ? (st.msg || '실행 중') : '대기');
  document.querySelectorAll('[data-bio-test^="mpop:"]').forEach(b => { b.disabled = running; });
}
$('mpopStop').addEventListener('click', () => api('mpop_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 자동 낚시 (제자리 낚시)
const mfish = () => (config.mfish ||= {});
const saveMfish = () => queueSave({ mfish: JSON.parse(JSON.stringify(mfish())) });
const MFISH_POS = [['fish_btn', 'Fish 버튼', 'Fish / Exit 버튼 가운데 (같은 자리)'], ['close_pos', '결과창 X', '낚시 결과창 오른쪽 위 X'],
  ['title_pos', '결과창 제목', '선택 · 제목 색으로 성공 / 쓰레기 / 실패 구분']];
const MFISH_REQ = ['fish_btn', 'bar_region', ...(window.acruxDesktopApi ? [] : ['close_pos'])];
const MFISH_TUNE = [['bite_max', '입질 최대 대기', '이 시간 동안 입질이 없으면 Exit 후 다시 던짐 (초)', 60, 5, 1],
  ['target_pct', '목표 위치', '구간 왼쪽 변에서 몇 % 지점에서 누를지 · 0 = 왼쪽 변, 50 = 가운데 (%)', 0, 0, 5],
  ['lead_ms', '미리 누르기', '이만큼 미리 누름 · 늦게 눌리면 조금 늘림 (ms)', 0, 0, 10],
  ['click_gap_ms', '클릭 최소 간격', '릴링 중 클릭 사이 최소 간격 (ms)', 45, 10, 5],
  ['cast_retry', 'Fish 다시 누르기', '반응이 없을 때 다시 누르는 횟수 · 넘으면 인벤토리 가득', 3, 1, 1]];
function renderMfish() {
  const m = mfish();
  $('mfishForm').innerHTML = onRow('mfish', '매크로가 켜져 있는 동안 낚시 장소에서 계속 낚시') + readyRow('mfish') + `
    <div class="row"><span>이번 실행 기록<small>성공 · 쓰레기 · 실패 · 인벤토리 가득</small></span><b id="mfishStats">-</b></div>`;
  syncReady('mfish');
  $('mfishTune').innerHTML = MFISH_TUNE.map(([k, name, sub, def, min, step]) => `
    <label class="row"><span>${name}<small>${sub} · 기본 ${def}</small></span>
      <input type="number" min="${min}" step="${step}" data-mfish-tune="${k}" value="${m[k] ?? def}"></label>`).join('');
  $('mfishTune').insertAdjacentHTML('beforeend', `
    <label class="row"><span>릴링 기록 저장<small>문제 확인용 기록 (데이터 폴더의 fishing_log.csv)</small></span>
      <span class="switch"><input type="checkbox" id="mfishDebug" ${m.debug_log ? 'checked' : ''}><i></i></span></label>`);
  $('mfishDebug').addEventListener('change', e => { m.debug_log = e.target.checked; saveMfish(); });
  $('mfishTune').querySelectorAll('[data-mfish-tune]').forEach(i => i.addEventListener('input', () => {
    const n = parseFloat(i.value);
    if (Number.isFinite(n) && n >= parseFloat(i.min)) { m[i.dataset.mfishTune] = n; saveMfish(); }
  }));
  if (lastMfish) updateMfish(lastMfish);
}
function updateMfish(st) {
  const running = !!(st && st.running);
  setData($('mfishDot'), 's', running ? 'flux' : '');
  setText($('mfishState'), running ? (st.msg || '실행 중') : '대기');
  const s = st && st.stats;
  if (s) setText($('mfishStats'), `${s.success} · ${s.junk} · ${s.fail} · ${s.full}`);
}
$('mfishStop').addEventListener('click', () => api('mfish_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 오토 아이템 사용
const mitem = () => (config.mitem ||= {});
const saveMitem = () => queueSave({ mitem: JSON.parse(JSON.stringify(mitem())) });
const MITEMS = [['strange', 'Strange Controller', 'strange_min', 10.5], ['randomizer', 'Biome Randomizer', 'randomizer_min', 18]];
const fmtLeft = s => s <= 0 ? '곧 사용' : s >= 60 ? `${Math.floor(s / 60)}분 ${Math.round(s % 60)}초 뒤` : `${Math.round(s)}초 뒤`;
function renderMitem() {
  const m = mitem();
  $('mitemForm').innerHTML = onRow('mitem', '매크로가 켜져 있는 동안 쿨타임마다 사용') + readyRow('mitem') + `
    <div class="row"><span>이번 실행 사용 횟수<small>Strange Controller · Biome Randomizer</small></span><b id="mitemUsed">-</b></div>`;
  syncReady('mitem');
  $('mitemSet').innerHTML = MITEMS.map(([k, name, mk, def]) => `
    <div class="row"><span>${name}<small>켜면 이 간격(분)마다 1개 사용 · <b data-mitem-left="${k}">-</b></small></span>
      <span class="pos"><input type="number" min="1" step="0.5" data-mitem-min="${mk}" value="${m[mk] ?? def}" title="분">
      <label class="switch"><input type="checkbox" data-mitem-on="${k}" ${m[k] !== false ? 'checked' : ''}><i></i></label></span></div>`).join('') + `
    <label class="row"><span>다 쓰고 Inventory 닫기<small>Inventory 버튼을 한 번 더 눌러 닫음</small></span>
      <span class="switch"><input type="checkbox" id="mitemClose" ${m.close_inventory !== false ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>인벤토리 위치 · OCR 영역<small>공통 위치 → 인벤토리 를 씀</small></span><span>공통 위치와 같음</span></div>`;
  $('mitemClose').addEventListener('change', e => { m.close_inventory = e.target.checked; saveMitem(); });
  $('mitemSet').querySelectorAll('[data-mitem-on]').forEach(i => i.addEventListener('change', () => { m[i.dataset.mitemOn] = i.checked; saveMitem(); }));
  $('mitemSet').querySelectorAll('[data-mitem-min]').forEach(i => i.addEventListener('input', () => {
    const n = parseFloat(i.value);
    if (Number.isFinite(n) && n >= 1) { m[i.dataset.mitemMin] = n; saveMitem(); }
  }));
  if (lastMitem) updateMitem(lastMitem);
}
function updateMitem(st) {
  const running = !!(st && st.running);
  setData($('mitemDot'), 's', running ? 'flux' : '');
  setText($('mitemState'), running ? (st.msg || '사용 중') : '대기');
  $('mitemTest').disabled = running;
  const it = (st && st.items) || {};
  document.querySelectorAll('[data-mitem-left]').forEach(el => {
    const x = it[el.dataset.mitemLeft];
    setText(el, !x ? '-' : !x.on ? '꺼짐' : !(config.macro_on && mitem().enabled) ? '매크로를 켜면 사용' : fmtLeft(x.left));
  });
  if (it.strange) setText($('mitemUsed'), `${it.strange.used} · ${it.randomizer.used}`);
}
$('mitemTest').addEventListener('click', async () => {
  const r = await api('mitem_test');
  toast(r.error || '오토 아이템 사용 테스트 시작 · 인벤토리 열기부터 · 정지: F7');
});
$('mitemStop').addEventListener('click', () => api('mitem_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 블러드 레인 자동 생성 (할로윈 이벤트)
// 내 서버에 PUMPKIN MOON · GRAVEYARD 가 뜨면 Cursed Rune Fragment 를 1개 사용 (2개 이상 있을 때만) · 위치는 공통 위치 → 인벤토리
const mblood = () => (config.mblood ||= {});
const saveMblood = () => queueSave({ mblood: JSON.parse(JSON.stringify(mblood())) });
function renderMblood() {
  const m = mblood();
  $('mbloodForm').innerHTML = onRow('mblood', 'PUMPKIN MOON · GRAVEYARD 가 뜨면 그 바이옴에 한 번 사용') + readyRow('mblood') + `
    <div class="row"><span>이번 실행 사용 횟수<small>Cursed Rune Fragment</small></span><b id="mbloodUsed">-</b></div>`;
  syncReady('mblood');
  $('mbloodSet').innerHTML = `
    <div class="row"><span>사용할 아이템<small>2개 이상 있을 때만 1개 사용 · 1개 이하면 안 씀</small></span><span>Cursed Rune Fragment</span></div>
    <div class="row"><span>바이옴<small>둘 중 하나가 뜨면 · 한 번 뜬 바이옴엔 한 번만 · 매크로를 바이옴 중간에 켜도 사용</small></span><span>PUMPKIN MOON · GRAVEYARD</span></div>
    <label class="row"><span>시작 전 대기<small>바이옴 감지 후 인벤토리를 열기까지 (초) · 기본 1</small></span>
      <input type="number" min="0" max="60" step="0.5" id="mbloodDelay" value="${m.start_delay ?? 1}"></label>
    <label class="row"><span>다 쓰고 인벤토리 닫기<small>Inventory 버튼을 한 번 더 눌러 닫음</small></span>
      <span class="switch"><input type="checkbox" id="mbloodClose" ${m.close_inventory !== false ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>버튼 위치 · OCR 영역<small>공통 위치 → 인벤토리 를 씀</small></span><span>공통 위치와 같음</span></div>
    <div class="row"><span>딜레이 · 이름 일치율<small>오토 팝핑 설정(스나이프 탭)에 지정한 것을 같이 씀</small></span><span>오토 팝핑과 같음</span></div>`;
  $('mbloodDelay').addEventListener('input', e => {
    const n = parseFloat(e.target.value);
    if (n >= 0 && n <= 60) { m.start_delay = n; saveMblood(); }
  });
  $('mbloodClose').addEventListener('change', e => { m.close_inventory = e.target.checked; saveMblood(); });
  if (lastMblood) updateMblood(lastMblood);
}
function updateMblood(st) {
  const running = !!(st && st.running);
  setData($('mbloodDot'), 's', running ? 'flux' : '');
  setText($('mbloodState'), running ? (st.msg || '실행 중') : '대기');
  $('mbloodTest').disabled = running;
  const used = $('mbloodUsed');
  if (used && st) setText(used, String(st.used || 0));
}
$('mbloodTest').addEventListener('click', async () => {
  const r = await api('mblood_test');
  toast(r.error || '블러드 레인 테스트 시작 · Cursed Rune Fragment 개수만 확인 (쓰지 않음) · 정지: F7');
});
$('mbloodStop').addEventListener('click', () => api('mblood_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 포션 자동 제작
const mcraft = () => (config.mcraft ||= {});
const saveMcraft = () => queueSave({ mcraft: JSON.parse(JSON.stringify(mcraft())) });
const craftPlaceOk = () => (move().places || []).some(pl => pl.feat === 'mcraft' && pl.points.length && pl.points.every(pt => pt.pos && pt.time != null));
// 제작할 포션: 직접 입력한 이름 → 게임에서 찾을 이름 (서버 crafter.typed_name 과 같게: 띄어쓰기 정리 · 끝 숫자 1~5 → 로마 숫자)
const ROMAN_NUM = { 1: 'I', 2: 'II', 3: 'III', 4: 'IV', 5: 'V' };
function potionName(v) {
  const w = String(v || '').split(/\s+/).filter(Boolean);
  if (w.length > 1 && ROMAN_NUM[w[w.length - 1]]) w[w.length - 1] = ROMAN_NUM[w[w.length - 1]];
  else if (w.length > 1 && Object.values(ROMAN_NUM).includes(w[w.length - 1].toUpperCase())) w[w.length - 1] = w[w.length - 1].toUpperCase();
  return w.join(' ');
}
function renderMcraft() {
  const m = mcraft();
  $('mcraftForm').innerHTML = onRow('mcraft', '매크로가 켜져 있는 동안 Auto Crafted 알림을 확인') + readyRow('mcraft') + `
    <div class="row"><span>이번 실행 제작 횟수</span><b id="mcraftCount">-</b></div>`;
  syncReady('mcraft');
  $('mcraftSet').innerHTML = `
    <div class="row potion-row"><span>제작할 포션<small>입력하면 알림이 뜰 때 이 포션을 채움 · 비워 두면 알림에서 포션 이름을 읽음 (OCR)</small></span>
      <span class="potion-in"><input type="text" id="mcraftPotion" placeholder="예: Fortune Potion I" maxlength="40" spellcheck="false" autocomplete="off">
      <span class="potion-mode" id="mcraftMode"></span></span></div>
    <label class="row"><span>F 누른 뒤 대기<small>제작 창이 뜰 때까지 (초)</small></span>
      <input type="number" min="0.5" step="0.5" id="mcraftWait" value="${m.f_wait ?? 2.5}"></label>
    <div class="row"><span>제작 테스트<small>위에 입력한 포션으로 지금 한 번 · 포션 제작 장소로 가서 재료를 채움</small></span>
      <button class="btn mini" type="button" id="mcraftTest">테스트</button></div>`;
  const inp = $('mcraftPotion');
  inp.value = m.potion || '';
  const showMode = () => {
    const n = potionName(inp.value);
    const chip = $('mcraftMode');
    chip.textContent = n ? '입력한 포션' : '알림에서 읽기 (OCR)';
    chip.title = n ? n : '';
    chip.classList.toggle('on', !!n);
  };
  showMode();
  inp.addEventListener('input', () => { m.potion = inp.value; saveMcraft(); showMode(); });
  inp.addEventListener('change', () => {           // 다 쓰고 나면 게임에서 찾을 이름으로 맞춰서 보여줌
    const n = potionName(inp.value);
    if (n !== inp.value) { inp.value = n; m.potion = n; saveMcraft(); showMode(); }
  });
  $('mcraftWait').addEventListener('input', e => {
    const n = parseFloat(e.target.value);
    if (Number.isFinite(n) && n >= 0.5) { m.f_wait = n; saveMcraft(); }
  });
  $('mcraftTest').addEventListener('click', async () => {
    const r = await api('mcraft_test', { name: potionName(inp.value) });
    toast(r.error || `제작 테스트 시작 — ${r.name} · 정지: F7`);
  });
  if (lastMcraft) updateMcraft(lastMcraft);
}
function updateMcraft(st) {
  const running = !!(st && st.running);
  setData($('mcraftDot'), 's', running ? 'flux' : '');
  setText($('mcraftState'), running ? (st.msg || '제작 중') : '대기');
  const t = $('mcraftTest');
  if (t) t.disabled = running;
  if (st) setText($('mcraftCount'), String(st.crafted ?? 0));
}
$('mcraftStop').addEventListener('click', () => api('mcraft_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 오토 메모리 매치
const mmatch = () => (config.mmatch ||= {});
const saveMmatch = () => queueSave({ mmatch: JSON.parse(JSON.stringify(mmatch())) });
const matchPlaceOk = () => (move().places || []).some(pl => pl.feat === 'mmatch' && pl.points.length && pl.points.every(pt => pt.pos && pt.time != null));
const fmtHM = s => s >= 3600 ? `${Math.floor(s / 3600)}시간 ${Math.floor(s % 3600 / 60)}분` : s >= 60 ? `${Math.floor(s / 60)}분` : `${s}초`;
function renderMmatch() {
  const m = mmatch();
  $('mmatchForm').innerHTML = onRow('mmatch', '매크로를 켤 때마다 한 번 확인 · 쿨타임이면 남은 시간 뒤에 다시') + readyRow('mmatch') + `
    <div class="row"><span>다음 확인</span><b id="mmatchNext">-</b></div>
    <div class="row"><span>이번 실행 횟수</span><b id="mmatchCount">-</b></div>
    <div class="row"><span>본 광고 (쿨타임 3시간 ↓)</span><b id="mmatchAds">-</b></div>`;
  syncReady('mmatch');
  $('mmatchSet').innerHTML = `
    <label class="row"><span>E 누른 뒤 대기<small>알림 창이 뜰 때까지 (초)</small></span>
      <input type="number" min="0.5" step="0.5" id="mmatchWait" value="${m.e_wait ?? 2}"></label>
    <label class="row"><span>카드 뒤집힌 뒤 대기<small>앞면을 읽기까지 (초)</small></span>
      <input type="number" min="0.1" step="0.05" id="mmatchFlip" value="${m.flip_wait ?? 0.35}"></label>
    <div class="row"><span>테스트<small>메모리 매치 보드 앞에서 지금 바로</small></span>
      <button class="btn mini" type="button" id="mmatchTest">테스트</button></div>`;
  [['mmatchWait', 'e_wait', 0.5], ['mmatchFlip', 'flip_wait', 0.1]].forEach(([id, k, lo]) => $(id).addEventListener('input', e => {
    const n = parseFloat(e.target.value);
    if (Number.isFinite(n) && n >= lo) { m[k] = n; saveMmatch(); }
  }));
  $('mmatchTest').addEventListener('click', async () => {
    const r = await api('mmatch_test');
    toast(r.error || '메모리 매치 테스트 시작 · 정지: F7');
  });
  if (lastMmatch) updateMmatch(lastMmatch);
}
function updateMmatch(st) {
  if (!st) return;
  const running = !!st.running;
  setData($('mmatchDot'), 's', running ? 'flux' : '');
  setText($('mmatchState'), running ? (st.msg || '메모리 매치 중') : '대기');
  const t = $('mmatchTest');
  if (t) t.disabled = running;
  setText($('mmatchCount'), String(st.played ?? 0));
  setText($('mmatchAds'), String(st.ads ?? 0));
  setText($('mmatchNext'), !mmatch().enabled ? '꺼짐' : running ? '지금' : st.next_in ? `${fmtHM(st.next_in)} 뒤` : '매크로를 켜면 바로');
}
$('mmatchStop').addEventListener('click', () => api('mmatch_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 상인 자동 구매
const mmerch = () => (config.mmerch ||= {});
const saveMmerch = () => queueSave({ mmerch: JSON.parse(JSON.stringify(mmerch())) });
const MERCH_ITEMS = { Jester: ['Oblivion Potion', 'Heavenly Potion', 'Potion of Bound', 'Rune of Everything', 'Random Potion Sack',
                               "Stella's Candle", 'Lucky Potion'],
                      Mari: ['Void Coin', 'Lucky Penny', 'Gear A', 'Gear B', 'Mixed Potion', 'Speed Potion', 'Lucky Potion', 'Fortune Spoid I'] };
const MERCH_NAME = { Jester: '제스터', Mari: '마리' };
const MMERCH_SET = [['check_sec', '채팅 확인 간격', '초', 15, 10, 5], ['teleport_wait', '순간이동 후 대기', '초', 3, 0.5, 0.5]];
const MMERCH_SHOP = ['first_slot', 'second_slot', 'item_region', 'max_pos', 'purchase_pos', 'close_pos'];
function renderMmerch() {
  const m = mmerch(), buy = (m.buy ||= {});
  $('mmerchForm').innerHTML = onRow('mmerch', '매크로가 켜져 있는 동안 채팅을 확인 · 상인이 오면 고른 아이템 구매') + readyRow('mmerch') + `
    <div class="row"><span>이번 실행 구매 횟수</span><b id="mmerchBought">-</b></div>`;
  syncReady('mmerch');
  $('mmerchSet').innerHTML = MMERCH_SET.map(([k, name, unit, def, min, step]) => `
    <label class="row"><span>${name}<small>${unit}</small></span>
      <input type="number" min="${min}" step="${step}" data-mmerch-set="${k}" value="${m[k] ?? def}"></label>`).join('') + `
    <div class="row"><span>구매 테스트<small>상인이 와 있을 때 Merchant Teleporter 부터 한 번</small></span>
      <span class="pos"><button class="btn mini ghost" type="button" data-mmerch-buy="Mari">마리</button>
      <button class="btn mini ghost" type="button" data-mmerch-buy="Jester">제스터</button></span></div>
    <div class="row"><span>대화창 · 인벤토리<small>공통 위치 → 대화창 · 인벤토리 를 씀</small></span><span>공통 위치와 같음</span></div>`;
  // 상인마다 따로 칸 (나란히 · 마리 → 제스터)
  $('mmerchItems').innerHTML = ['Mari', 'Jester'].map(who => [who, MERCH_ITEMS[who]]).map(([who, list]) => `
    <div class="card form merch-card"><h3>${MERCH_NAME[who]}</h3>` + list.map(n => {
    const k = `${who}_${n}`, on = buy[k] > 0;
    return `
      <div class="row"><span>${n}</span>
        <label class="switch"><input type="checkbox" data-mmerch-item="${k}" ${on ? 'checked' : ''}><i></i></label></div>`;
  }).join('') + `
    </div>`).join('');
  $('mmerchSet').querySelectorAll('[data-mmerch-set]').forEach(i => i.addEventListener('input', () => {
    const n = parseFloat(i.value);
    if (Number.isFinite(n) && n >= +i.min) { m[i.dataset.mmerchSet] = n; saveMmerch(); }
  }));
  $('mmerchSet').querySelectorAll('[data-mmerch-buy]').forEach(b => b.addEventListener('click', async () => {
    const r = await api('mmerch_buy_test', { name: b.dataset.mmerchBuy });
    toast(r.error || '구매 테스트 시작 · 정지: F7');
  }));
  $('mmerchItems').querySelectorAll('[data-mmerch-item]').forEach(i => i.addEventListener('change', () => {
    const k = i.dataset.mmerchItem;
    if (i.checked) buy[k] = 1; else delete buy[k];
    saveMmerch(); syncMerchPick(); syncReady('mmerch'); Tutorial.refresh();
  }));
  syncMerchPick();
  if (lastMmerch) updateMmerch(lastMmerch);
}
// 살 아이템 제목 옆: 고른 개수
function syncMerchPick() {
  const n = Object.values(mmerch().buy || {}).filter(v => v > 0).length;
  const el = $('mmerchPick');
  el.textContent = n ? `${n}개 고름` : '고른 아이템 없음';
  el.classList.toggle('done', n > 0);
}
function updateMmerch(st) {
  const running = !!(st && st.running);
  setData($('mmerchDot'), 's', running ? 'flux' : '');
  setText($('mmerchState'), running ? (st.msg || '작동 중') : '대기');
  $('mmerchCheck').disabled = running;
  if (st) setText($('mmerchBought'), String(st.bought ?? 0));
}
$('mmerchCheck').addEventListener('click', async () => {
  const r = await api('mmerch_check');
  toast(r.error || '채팅 확인 중 · 상인이 있으면 구매까지 이어감');
});
$('mmerchStop').addEventListener('click', () => api('mmerch_stop'));

// ---------------------------------------------------------------- 매크로 기능 켜기 · 끄기
// 기능 칸 맨 위 [켜기] · 메인 화면 매크로 ▾ 는 전부 같은 설정 (data-feat 스위치)
const FEATS = ['mpop', 'mfish', 'mitem', 'mmerch', 'mcraft', 'mmatch', 'mblood'];
const FEAT_NAME = { mpop: '레어 바이옴 자동 팝핑', mfish: '자동 낚시', mitem: '오토 아이템 사용', mmerch: '상인 자동 구매', mcraft: '포션 자동 제작', mmatch: '오토 메모리 매치',
  mblood: '블러드 레인 자동 생성' };
const FEAT_SEC = { mpop: 'mf-pop', mfish: 'mf-fish', mitem: 'mf-item', mmerch: 'mf-merchant', mcraft: 'mf-craft', mmatch: 'mf-memory', mblood: 'mf-blood' };
const featCfg = k => ({ mpop, mfish, mitem, mmerch, mcraft, mmatch, mblood, base })[k]();
function setFeature(k, on, quiet) {
  const c = featCfg(k);
  c.enabled = !!on;
  queueSave({ [k]: JSON.parse(JSON.stringify(c)) });
  if (k === 'mfish' && !on) api('mfish_stop');
  if (k === 'mmerch' && !on) api('mmerch_stop');
  if (k === 'mcraft' && !on) api('mcraft_stop');
  if (k === 'mmatch' && !on) api('mmatch_stop');
  // 다시 그리지 않고 같은 기능의 스위치만 맞춤 (다시 그리면 누른 스위치가 새로 생겨서 움직이는 애니메이션이 안 보임)
  document.querySelectorAll(`[data-feat="${k}"]`).forEach(i => { i.checked = !!on; });
  syncMainTiles();
  Quick.sync();
  if (k === 'mmatch' && lastMmatch) updateMmatch(lastMmatch);
  if (!quiet) toast(`${FEAT_NAME[k]} ${on ? '켜짐' : '꺼짐'}`);
  if (on && !quiet) Tutorial.featureOn(k);
}
function setAllFeatures(on) {
  FEATS.forEach(k => setFeature(k, on, true));
  toast(on ? '기능 전부 켜짐' : '기능 전부 꺼짐');
}
document.addEventListener('change', e => {
  const k = e.target.dataset?.feat;
  if (k && FEATS.includes(k)) setFeature(k, e.target.checked);
});

// ---------------------------------------------------------------- 매크로 설정 · 위치 (공통 위치 칸 · 기능마다 위치 · 이동 묶음)
// feat: 설정 묶음 (base = 여러 기능이 같이 쓰는 공통 위치 / mfish = 자동 낚시만 …) · points: [키, 이름, 설명] · region: [키, 이름, 설명]
// 저장은 서버(api)에서 · windows: 창 영역(드래그) → 안쪽 위치를 서버가 계산
const MPOS = {
  base: { box: 'mposBase', link: true,
          points: [...POP_POS.map(([k, n, sub]) => [k, n, sub || '']),
                   ['chat_pos', '채팅 버튼', '왼쪽 위의 채팅 버튼'],
                   ['collection_pos', '도감 버튼', '왼쪽 메뉴의 도감(Collection · 책 모양) 버튼'],
                   ['collection_close', '도감 Exit', '도감을 연 상태에서 Exit(닫기) 버튼'],
                   ['dialog_pos', '대화창', 'NPC 대화를 넘길 곳 (Click to skip.)']],
          regions: [['ocr_region', 'OCR 영역', '검색 결과 아이템 이름·개수 (예: Warp Potion x23)'],
                    ['notice_region', '알림 영역', '오른쪽 알림 카드 자리를 넉넉히 드래그']],
          // 칸: [id, 이름, 설명, 들어갈 항목, 맨 위에 연동 줄을 넣을지, 따로 그리는 부분(MPOS_CUSTOM)]
          tabs: [['inv', '인벤토리', '쓰는 기능: 레어 바이옴 자동 팝핑 · 오토 아이템 사용 · 상인 자동 구매 · 블러드 레인 자동 생성 (스나이프 오토 팝핑과 연동 가능)',
                  [...POP_POS.map(([k]) => k), 'ocr_region'], true],
                 ['notice', '알림', '쓰는 기능: 자동 낚시 (인벤토리 가득) · 포션 자동 제작 (Auto Crafted)', ['notice_region']],
                 ['ui', '대화창', '쓰는 기능: 자동 낚시 (판매) · 상인 자동 구매', ['dialog_pos']],
                 ['move', '이동 · 기준 장소', '모든 이동의 출발점 (리셋 → 카메라 정렬 → 걷기 → 줌) · 쓰는 기능: 자동 낚시 · 포션 자동 제작 · 오토 메모리 매치',
                  ['chat_pos', 'collection_pos', 'collection_close'], false, 'movebase']] },
  mfish: { box: 'mposFish',
           windows: [['panel_region', '낚시 대기 창 영역', 'Fish 버튼이 보일 때 창 테두리까지 드래그'],
                     ['reel_region', '낚시 미니게임 창 영역', '미니게임 중에 창 테두리까지 드래그'],
                     ['result_region', '결과창 영역', '결과창이 떠 있을 때 창 테두리까지 드래그']],
           points: [...MFISH_POS,
                    ['sell_fish_pos', 'Sell Fish 버튼', '대화 선택지 중 파란 [Sell Fish]'],
                    ['first_fish_pos', '첫 번째 물고기 칸', '상점 목록의 맨 앞 칸'],
                    ['sell_all_pos', 'Sell All 버튼', '왼쪽 물고기 정보 아래'],
                    ['confirm_sell_pos', '확인 Sell 버튼', '확인창의 초록 Sell'],
                    ['shop_close_pos', '상점 닫기 X', '상점 오른쪽 위 X']],
           regions: [['bar_region', '릴링 바 영역', '위쪽 바만 딱 맞게 드래그']],
           tabs: [['win', '창 영역', '창 테두리만 드래그하면 안쪽 위치는 자동 계산',
                   ['panel_region', 'reel_region', 'result_region'], false, '', 'fishauto'],
                  ['fine', '세부 위치', '창 영역으로 자동 계산됨 · 조금 어긋나면 여기서 하나씩 직접 지정',
                   ['fish_btn', 'close_pos', 'title_pos', 'bar_region']],
                  ['sell', '판매', '인벤토리가 가득 차면 물고기를 팔고 돌아옴',
                   ['sell_fish_pos', 'first_fish_pos', 'sell_all_pos', 'confirm_sell_pos', 'shop_close_pos'], true, 'sell', 'sellauto'],
                  ['move', '이동', '매크로를 켜면 낚시 장소로, 가득 차면 판매 장소로 이동 · 기준 장소(공통 위치 → 이동 · 기준 장소)에서 출발', [], false, 'places:mfish']] },
};
MPOS.mcraft = { box: 'mposCraft',
  points: [['search_pos', '검색창', '제작 창 오른쪽 위 Search'], ['shop_close_pos', '제작 창 X', "Stella's Workshop 오른쪽 위 X"],
           ['open_recipe_pos', 'Open Recipe 버튼', '왼쪽 아래 Open Recipe'], ['add_all_pos', 'Add Everything 버튼', 'Add Ingredients 창 왼쪽 아래'],
           ['craft_pos', 'Craft 버튼', 'Add Ingredients 창 오른쪽 아래'], ['add_close_pos', 'Add Ingredients 창 X', 'Add Ingredients 오른쪽 위 X']],
  regions: [['list_region', '검색 결과 목록', '검색창 아래 포션 목록 전체']],
  tabs: [['shop', '제작 창', '실행할 땐 글자로 먼저 찾고, 못 찾으면 이 위치를 씀',
          ['search_pos', 'list_region', 'shop_close_pos', 'open_recipe_pos', 'add_all_pos', 'craft_pos', 'add_close_pos'], false, '', 'craftauto'],
         ['revbase', '반대쪽 기준 장소', '낚시와 정반대 방향 · 리셋 → 카메라 정렬 → 내려다보기+줌 → S+D → S (끝에 A 같이)', [], false, 'movebaserev'],
         ['move', '이동', '1번 퀘스트 보드 → E → 대기 → Exit → D → 2번 스텔라 포탈 → D = 포션 제작 장소', [], false, 'places:mcraft']] };
MPOS.mmatch = { box: 'mposMatch',
  points: [['note_close_pos', '알림 창 X', 'E 를 누르면 뜨는 Notification 창 오른쪽 위 X'],
           ['note_btn_pos', 'Start / Available 버튼', '창 가운데 초록 버튼 (Start Memory Match · Available after)'],
           ['ad_pos', 'Watch AD 칸', '창 아래 Watch AD to lower the cooltime'],
           ['board_close_pos', '카드 판 Close 버튼', '게임이 끝나면 카드 판 아래 뜨는 Close']],
  regions: [['note_region', '알림 창 영역', 'Notification 창부터 Watch AD 칸까지 (이 영역만 읽어서 빠름)']],
  tabs: [['note', '메모리 매치 창', '지정한 위치를 누름 · 비어 있으면 글자로 찾은 자리를 누름',
          ['note_region', 'note_close_pos', 'note_btn_pos', 'ad_pos'], false, '', 'matchauto'],
         ['board', '카드 판', '카드 판이 떠 있을 때 자동 보정하면 카드 자리를 그 위치로 씀 · 안 하면 글자로 찾음',
          ['board_close_pos'], false, '', 'boardauto'],
         ['move', '이동', '기준 장소 → 메모리 매치 보드 앞 (E 를 누를 수 있는 곳)', [], false, 'places:mmatch']] };
MPOS.mmerch = { box: 'mposMerch',
  points: [['open_pos', 'Open 선택지', '대화 선택지 Open (글자로 못 찾을 때만)'],
           ['first_slot', '첫 번째 칸', '상점 맨 왼쪽 아이템 칸'],
           ['second_slot', '두 번째 칸', '그 옆 칸 (칸 간격 계산)'],
           ['max_pos', 'Set to Max 버튼', '수량 칸 오른쪽 Set to Max'],
           ['purchase_pos', 'Purchase 버튼', '수량 칸 아래 Purchase'],
           ['close_pos', '상점 닫기 X', '상점 오른쪽 위 X']],
  regions: [['chat_region', '채팅 글자 영역', '채팅 글자가 보이는 곳 전체'],
            ['item_region', '아이템 이름 영역', '칸을 눌렀을 때 뜨는 아이템 이름']],
  tabs: [['chat', '채팅', '상인 도착 감지 (채팅 OCR)', ['chat_region'], true],
         ['shop', '상점', '자동 보정이 켜져 있으면 비워 둬도 됨',
          ['open_pos', 'first_slot', 'second_slot', 'item_region', 'max_pos', 'purchase_pos', 'close_pos'], false, '', 'merchauto']] };
function renderMpos(feat) {
  const d = MPOS[feat], c = featCfg(feat), box = $(d.box);
  const regions = [...(d.windows || []), ...d.regions];
  const row = (k, name, sub, val, fmt, btn, attr) => `
    <div class="row"><span>${name}<small>${sub}</small></span>
      <span class="pos"><code class="${val ? '' : 'unset'}">${fmt(val)}</code>
      <button class="btn mini ghost" type="button" ${attr}="${k}">${btn}</button></span></div>`;
  const items = {};
  d.points.forEach(([k, name, sub]) => { items[k] = row(k, `${name} 위치`, sub, c[k], fmtPos, '위치 지정', 'data-mpos-pick'); });
  regions.forEach(([k, name, sub]) => { items[k] = row(k, name, sub, c[k], fmtReg, '드래그로 지정', 'data-mpos-region'); });
  const link = d.link ? `
    <label class="row"><span>스나이프 오토 팝핑과 연동<small>켜면 스나이프 탭 오토 팝핑도 이 위치를 씀</small></span>
      <span class="switch"><input type="checkbox" data-mpos-link ${pop().use_base ? 'checked' : ''}><i></i></span></label>` : '';
  // 묶음마다 따로 칸(카드)으로 나눠서 보여 줌 · 제목 옆에 지정한 개수 / 전체 개수
  box.innerHTML = d.tabs.map(([id, name, sub, keys, top, custom, pre]) => {
    const n = keys.filter(k => c[k]).length;
    return `
    <div class="grp mpos-group" data-mpos-group="${id}">
      <div class="grp-head"><b>${name}</b>${keys.length ? `<em class="${n === keys.length ? 'done' : ''}">${n}/${keys.length}</em>` : ''}</div>
      <p class="grp-desc">${sub}</p>
      <div class="card form mpos-card">${pre ? `<div data-mpos-custom="${pre}"></div>` : ''}${top ? link : ''}${keys.map(k => items[k] || '').join('')}${custom ? `<div data-mpos-custom="${custom}"></div>` : ''}</div></div>`;
  }).join('');
  box.querySelectorAll('[data-mpos-custom]').forEach(renderMoveCustom);
  box.querySelector('[data-mpos-link]')?.addEventListener('change', e => setPopLink(e.target.checked));
  box.querySelectorAll('[data-mpos-pick]').forEach(b => b.addEventListener('click', async () => {
    const k = b.dataset.mposPick, name = d.points.find(x => x[0] === k)[1];
    const r = await pickWith(b, `로블록스 화면에서 ${name} 클릭`, () => api('mpos_point', { feat, key: k }));
    if (r) { c[k] = r.pos; mposChanged(feat); toast(`${name} 위치 저장`); }
  }));
  box.querySelectorAll('[data-mpos-region]').forEach(b => b.addEventListener('click', async () => {
    const k = b.dataset.mposRegion, name = regions.find(x => x[0] === k)[1];
    const r = await pickWith(b, `로블록스 화면에서 ${name} 드래그`, () => api('mpos_region', { feat, key: k }));
    if (r) { Object.assign(c, r[feat] || { [k]: r.region }); mposChanged(feat); toast(`${name} 저장`); }
  }));
}
// ---------------------------------------------------------------- 이동 (공통 위치 → 이동 · 기준 장소 / 기능마다 장소)
// 기준 장소: 리셋 → Collection 열고 닫기(카메라 정렬) → 줌 · 장소: 지점 목록 [누를 곳, 걸린 시간]
const move = () => (config.move ||= { places: [] });
const saveMove = () => queueSave({ move: JSON.parse(JSON.stringify(move())) });
const MOVE_SET = [['reset_wait', '리셋 후 대기', 'Esc → R → Enter 로 리셋한 뒤 (초)', 3.5, 0.5, 0.1],
                  ['w_time', 'W 누르기', 'W 만 누르는 시간 (초)', 1, 0, 0.05],
                  ['wa_time', 'W + A 누르기', 'W 와 A 를 같이 누르는 시간 (초)', 7, 0, 0.5],
                  ['a_time', 'A 누르기', '그 다음 A 를 누르는 시간 (초)', 1, 0, 0.05],
                  ['w2_time', 'W 같이 누르기', 'A 를 누르는 마지막 이 시간 동안 W 도 같이 (초)', 0.25, 0, 0.05],
                  ['sd_time', 'S + D 누르기', '그 다음 S 와 D 를 같이 누르는 시간 (초)', 0.25, 0, 0.05],
                  ['tilt_px', '화면 내려다보기', '우클릭을 누른 채 마우스를 아래로 끄는 거리 (px)', 800, 0, 50],
                  ['o_time', 'O 누르기', '최대 줌 (초)', 2.5, 0, 0.1],
                  ['margin', '도착 여유', '잰 시간에 더 기다릴 시간 (초)', 0.3, 0, 0.1]];
const REV_SET = [['rsd_time', 'S + D 누르기', 'S 와 D 를 같이 누르는 시간 (초)', 1, 0, 0.1],
                 ['rs_time', 'S 누르기', '그 다음 S 를 누르는 시간 (초)', 6, 0, 0.5],
                 ['ra_time', 'A 같이 누르기', 'S 를 누르는 마지막 이 시간 동안 A 도 같이 (초)', 1, 0, 0.1],
                 ['q_wait', '퀘스트 보드 E 뒤 대기', 'E 를 누르고 Exit 를 누르기까지 (초)', 1.5, 0, 0.1],
                 ['qd_time', 'Exit 뒤 D 누르기', '퀘스트 창 Exit 를 누른 뒤 D 를 누르는 시간 (초)', 2, 0, 0.1],
                 ['qd_wait', 'D 뒤 대기', 'Exit 뒤 D 를 누른 다음 기다리는 시간 (초)', 0.65, 0, 0.05],
                 ['rd_time', '포탈 뒤 D 누르기', '스텔라 포탈 다음 D 를 누르는 시간 (초) — 끝나는 자리가 포션 제작 장소', 2, 0, 0.1]];
const revTime = m => 4.4 + (m.reset_wait ?? 3.5) + Math.max(m.o_time ?? 2.5, (m.tilt_px ?? 800) / 20 * 0.015)
  + (m.rsd_time ?? 1) + Math.max(m.rs_time ?? 6, m.ra_time ?? 1);
const questTime = m => 0.6 + (m.q_wait ?? 1.5) + (m.qd_time ?? 2) + (m.qd_wait ?? 0.65) + (m.rd_time ?? 2);
let lastMove = null;
const fmtT = t => t == null ? '안 잼' : `${t}초`;
// 장소의 지점이 전부 지정 · 측정되면 제목 옆에 총 걸리는 시간 — Esc(리셋)부터 도착까지 전부
// 기준 장소 (서버 move.go_base 순서): 0.2 + Esc·R 1.0 + 리셋 대기 + 카메라 정렬 2.8 + W + W+A + 0.5 + 내려다보기·줌 + 0.4
const baseTime = m => 4.9 + (m.reset_wait ?? 3.5) + (m.w_time ?? 1) + (m.wa_time ?? 7) + Math.max(m.a_time ?? 1, m.w2_time ?? 0.25) + (m.sd_time ?? 0.25)
  + Math.max(m.o_time ?? 2.5, (m.tilt_px ?? 800) / 20 * 0.015);
const placeTotal = pl => {
  if (!pl.points.length || !pl.points.every(pt => pt.pos && pt.time != null)) return '';
  const rev = pl.feat === 'mcraft';
  const m = move(), t = (rev ? revTime(m) + questTime(m) : baseTime(m)) + pl.points.reduce((a, pt) => a + pt.time + (m.margin ?? 0.3), 0);
  return `<em class="move-total">총 ${Math.round(t * 10) / 10}초</em>`;
};
const MOVE_FIXED = ['mfish', 'mcraft', 'mmatch'];                            // 자동 낚시: 낚시 장소 · 물고기 판매 장소 (서버 MOVE_TEMPLATES)
const moveCall = async (name, args, msg) => { const r = await api(name, args); toast(r.error || msg); return r; };
// data-mpos-custom: 'movebase' (공통 위치 → 기준 장소) / 'places:기능' (그 기능이 가는 장소)
function renderMoveCustom(el) {
  const [kind, feat] = el.dataset.mposCustom.split(':');
  if (kind === 'movebase') MPOS_CUSTOM.movebase(el);
  else if (kind === 'sell') MPOS_CUSTOM.sell(el);
  else if (kind === 'fishauto' || kind === 'sellauto' || kind === 'merchauto' || kind === 'craftauto' || kind === 'matchauto' || kind === 'boardauto' || kind === 'movebaserev') MPOS_CUSTOM[kind](el);
  else MPOS_CUSTOM.places(el, feat);
}
function rerenderMove() { document.querySelectorAll('[data-mpos-custom]').forEach(renderMoveCustom); syncAllReady(); Jump.refresh(); Tutorial.refresh(); }
const SELL_SET = [['sell_delay', '클릭 사이 추가 대기', '렉이 있으면 늘림 (초)', 0, 0, 0.1],
                  ['sell_max', '판매 반복 최대', '클릭이 씹혀 끝없이 도는 것만 막음', 100, 1, 1]];
let sellcalBusy = false;
let autocalBusy = false;
let merchcalBusy = false;
let craftcalBusy = false;
let matchcalBusy = false;
let boardcalBusy = false;
const autoRow = (name, sub, busy, label, attr) => `
      <div class="row"><span>${name}<small>${sub}</small></span>
        <span class="pos"><button class="btn mini ${busy ? 'waiting' : ''}" type="button" ${attr}>${busy ? '보정 중… (다시 누르면 취소)' : label}</button></span></div>`;
const MPOS_CUSTOM = {
  // 자동 보정 (칸 맨 위): 낚시 창 · 판매 위치
  fishauto(el) {
    el.innerHTML = autoRow('자동 보정', 'Fish 버튼이 보일 때 누르면 위치를 전부 맞춤', autocalBusy, '자동 보정', 'data-fish-auto');
    el.querySelector('[data-fish-auto]').addEventListener('click', async () => {
      if (autocalBusy) { const r = await api('mfish_autocal'); return r.error && toast(r.error); }
      autocalBusy = true; rerenderMove();
      try {
        const r = await api('mfish_autocal');
        if (r.mfish) { Object.assign(mfish(), r.mfish); mposChanged('mfish'); }
        toast(r.error || `자동 보정 완료: ${r.done}`);
      } catch (err) { toast('자동 보정 실패: ' + err.message); }
      finally { autocalBusy = false; rerenderMove(); }
    });
  },
  sellauto(el) {
    el.innerHTML = autoRow('판매 자동 보정', '누른 뒤 Captain Flarg 앞에서 E · 실제로 팔지는 않음', sellcalBusy, '판매 자동 보정', 'data-sell-auto');
    el.querySelector('[data-sell-auto]').addEventListener('click', async () => {
      if (sellcalBusy) { const r = await api('sell_autocal'); return r.error && toast(r.error); }
      sellcalBusy = true; rerenderMove();
      toast('Captain Flarg 앞에서 E 를 눌러 주세요');
      try {
        const r = await api('sell_autocal');
        if (r.base) { config.base = r.base; mposChanged('base'); }
        if (r.mfish) { Object.assign(mfish(), r.mfish); mposChanged('mfish'); }
        toast(r.error || `판매 자동 보정 완료${r.notes?.length ? ' · ' + r.notes.join(' · ') : ''}`);
      } catch (err) { toast('판매 자동 보정 실패: ' + err.message); }
      finally { sellcalBusy = false; rerenderMove(); }
    });
  },
  // 상인: 상점 위치 자동 보정 (구매할 때마다) · E 를 눌러 지금 보정
  merchauto(el) {
    const m = mmerch();
    el.innerHTML = `
      <label class="row"><span>상점 위치 자동 보정<small>순간이동 후 상점이 열리면 화면 글자로 위치를 맞춤</small></span>
        <span class="switch"><input type="checkbox" data-merch-cal ${m.auto_cal !== false ? 'checked' : ''}><i></i></span></label>` +
      autoRow('지금 자동 보정', '상인이 와 있을 때 누른 뒤 상인 앞에서 E · 사지는 않음', merchcalBusy, '자동 보정', 'data-merch-auto');
    el.querySelector('[data-merch-cal]').addEventListener('change', e => {
      m.auto_cal = e.target.checked; saveMmerch();
      setTimeout(() => syncAllReady(), 230);
    });
    el.querySelector('[data-merch-auto]').addEventListener('click', async () => {
      if (merchcalBusy) { const r = await api('mmerch_autocal'); return r.error && toast(r.error); }
      merchcalBusy = true; rerenderMove();
      toast('상인 앞에서 E 를 눌러 주세요');
      try {
        const r = await api('mmerch_autocal');
        if (r.mmerch) { Object.assign(mmerch(), r.mmerch); mposChanged('mmerch'); }
        toast(r.error || '상인 자동 보정 완료');
      } catch (err) { toast('상인 자동 보정 실패: ' + err.message); }
      finally { merchcalBusy = false; rerenderMove(); }
    });
  },
  // 포션 제작: F 를 눌러 제작 창 위치 보정
  craftauto(el) {
    el.innerHTML = autoRow('자동 보정', '누른 뒤 Stella 앞에서 F · 재료는 안 넣음', craftcalBusy, '자동 보정', 'data-craft-auto');
    el.querySelector('[data-craft-auto]').addEventListener('click', async () => {
      if (craftcalBusy) { const r = await api('mcraft_autocal'); return r.error && toast(r.error); }
      craftcalBusy = true; rerenderMove();
      toast('Stella 앞에서 F 를 눌러 주세요');
      try {
        const r = await api('mcraft_autocal');
        if (r.mcraft) { Object.assign(mcraft(), r.mcraft); mposChanged('mcraft'); }
        toast(r.error || '포션 제작 자동 보정 완료');
      } catch (err) { toast('포션 제작 자동 보정 실패: ' + err.message); }
      finally { craftcalBusy = false; rerenderMove(); }
    });
  },
  // 메모리 매치: E 를 눌러 알림 창 · 카드 판 위치 보정
  matchauto(el) {
    el.innerHTML = autoRow('자동 보정', '누른 뒤 메모리 매치 보드 앞에서 E · Start · 광고는 안 누름', matchcalBusy, '자동 보정', 'data-match-auto');
    el.querySelector('[data-match-auto]').addEventListener('click', async () => {
      if (matchcalBusy) { const r = await api('mmatch_autocal'); return r.error && toast(r.error); }
      matchcalBusy = true; rerenderMove();
      toast('메모리 매치 보드 앞에서 E 를 눌러 주세요');
      try {
        const r = await api('mmatch_autocal');
        if (r.mmatch) { Object.assign(mmatch(), r.mmatch); mposChanged('mmatch'); }
        toast(r.error || `메모리 매치 자동 보정 완료${r.notes?.length ? ' · ' + r.notes.join(' · ') : ''}`);
      } catch (err) { toast('메모리 매치 자동 보정 실패: ' + err.message); }
      finally { matchcalBusy = false; rerenderMove(); }
    });
  },
  // 메모리 매치 카드 판: 판이 떠 있을 때 카드 자리 보정
  boardauto(el) {
    el.innerHTML = autoRow('카드 판 자동 보정', '메모리 매치를 시작해서 카드 판이 떠 있을 때 누르기', boardcalBusy, '자동 보정', 'data-board-auto');
    el.querySelector('[data-board-auto]').addEventListener('click', async () => {
      if (boardcalBusy) return;
      boardcalBusy = true; rerenderMove();
      try {
        const r = await api('mmatch_boardcal');
        if (r.mmatch) { Object.assign(mmatch(), r.mmatch); mposChanged('mmatch'); }
        toast(r.error || '카드 판 자동 보정 완료');
      } catch (err) { toast('카드 판 자동 보정 실패: ' + err.message); }
      finally { boardcalBusy = false; rerenderMove(); }
    });
  },
  // 판매: 대기 · 반복 설정 + [판매 테스트]
  sell(el) {
    const m = mfish(), st = lastMove || {}, busy = !!st.running;
    el.innerHTML = SELL_SET.map(([k, name, sub, def, min, step]) => `
      <label class="row"><span>${name}<small>${sub} · 기본 ${def}</small></span>
        <input type="number" min="${min}" step="${step}" data-sell-set="${k}" value="${m[k] ?? def}"></label>`).join('') + `
      <div class="row"><span>판매 테스트<small>기준 장소 → 물고기 판매 장소 → 판매 → 낚시 장소 · 정지: F7</small></span>
        <span class="pos"><b class="move-state">${esc(busy ? (st.msg || '이동 중') : '대기')}</b>
        <button class="btn mini" type="button" data-sell-test ${busy ? 'disabled' : ''}>판매 테스트</button>
        <button class="btn mini ghost" type="button" data-move-stop ${busy ? '' : 'disabled'}>멈춤</button></span></div>`;
    el.querySelectorAll('[data-sell-set]').forEach(i => i.addEventListener('input', () => {
      const n = parseFloat(i.value);
      if (!isNaN(n) && n >= 0) { m[i.dataset.sellSet] = n; saveMfish(); }
    }));
    el.querySelector('[data-sell-test]').addEventListener('click', async () => {
      await flushSave();
      moveCall('sell_test', {}, '판매 테스트 시작 · 정지: F7');
    });
    el.querySelector('[data-move-stop]').addEventListener('click', () => api('move_stop'));
  },
  // 기준 장소 (모든 기능이 같이 씀): 리셋 · 카메라 정렬 · 줌 설정 + [기준 장소로 이동]
  movebase(el) {
    const m = move(), st = lastMove || {}, busy = !!st.running;
    el.innerHTML = `
      <div class="row"><span>기준 장소로 이동<small>위 순서대로 테스트</small></span>
        <span class="pos"><b class="move-state">${esc(busy ? (st.msg || '이동 중') : '대기')}</b>
        <button class="btn mini" type="button" data-move-base ${busy ? 'disabled' : ''}>기준 장소로 이동</button>
        <button class="btn mini ghost" type="button" data-move-stop ${busy ? '' : 'disabled'}>멈춤</button></span></div>` +
      MOVE_SET.map(([k, name, sub, def, min, step]) => `
      <label class="row"><span>${name}<small>${sub} · 기본 ${def}</small></span>
        <input type="number" min="${min}" step="${step}" data-move-set="${k}" value="${m[k] ?? def}"></label>`).join('') + `
      <label class="row"><span>누를 버튼<small>장소를 누를 마우스 버튼 (Click to Move)</small></span>
        <select data-move-btn><option value="right">우클릭</option><option value="left">좌클릭</option></select></label>`;
    const sel = el.querySelector('[data-move-btn]');
    sel.value = m.button || 'right';
    sel.addEventListener('change', () => { m.button = sel.value; saveMove(); });
    el.querySelectorAll('[data-move-set]').forEach(i => i.addEventListener('input', () => {
      const n = parseFloat(i.value);
      if (!isNaN(n) && n >= 0) {
        m[i.dataset.moveSet] = n; saveMove();
        document.querySelectorAll('[data-mpos-custom^="places"]').forEach(renderMoveCustom);   // 총 시간 다시 계산
      }
    }));
    el.querySelector('[data-move-base]').addEventListener('click', () => moveCall('move_base', {}, '기준 장소로 이동 시작 · 정지: F7'));
    el.querySelector('[data-move-stop]').addEventListener('click', () => api('move_stop'));
  },
  // 반대쪽 기준 장소 (포션 제작 장소 출발점) · 퀘스트 보드 동작
  movebaserev(el) {
    const m = move(), st = lastMove || {}, busy = !!st.running;
    el.innerHTML = `
      <div class="row"><span>반대쪽 기준 장소로 이동<small>위 순서대로 테스트</small></span>
        <span class="pos"><b class="move-state">${esc(busy ? (st.msg || '이동 중') : '대기')}</b>
        <button class="btn mini" type="button" data-move-rev ${busy ? 'disabled' : ''}>반대쪽 기준 장소로 이동</button>
        <button class="btn mini ghost" type="button" data-move-stop ${busy ? '' : 'disabled'}>멈춤</button></span></div>` +
      REV_SET.map(([k, name, sub, def, min, step]) => `
      <label class="row"><span>${name}<small>${sub} · 기본 ${def}</small></span>
        <input type="number" min="${min}" step="${step}" data-move-set="${k}" value="${m[k] ?? def}"></label>`).join('') + `
      <div class="row"><span>퀘스트 보드 Exit 위치<small>퀘스트 보드 앞에서 E 로 창을 연 뒤 [바로 지정] → Exit 클릭</small></span>
        <span class="pos"><code class="${m.quest_exit_pos ? '' : 'unset'}">${fmtPos(m.quest_exit_pos)}</code>
        <button class="btn mini ghost" type="button" data-quest-pick>바로 지정</button></span></div>`;
    el.querySelectorAll('[data-move-set]').forEach(i => i.addEventListener('input', () => {
      const n = parseFloat(i.value);
      if (!isNaN(n) && n >= 0) {
        m[i.dataset.moveSet] = n; saveMove();
        document.querySelectorAll('[data-mpos-custom^="places"]').forEach(renderMoveCustom);
      }
    }));
    el.querySelector('[data-move-rev]').addEventListener('click', () => moveCall('move_base_rev', {}, '반대쪽 기준 장소로 이동 시작 · 정지: F7'));
    el.querySelector('[data-move-stop]').addEventListener('click', () => api('move_stop'));
    el.querySelector('[data-quest-pick]').addEventListener('click', async e => {
      const r = await pickWith(e.currentTarget, '로블록스 화면에서 퀘스트 창 Exit 클릭', () => api('move_quest_pick'));
      if (r && r.move) { config.move = r.move; rerenderMove(); toast('퀘스트 보드 Exit 위치 저장'); }
    });
  },
  // 기능마다 가는 장소: 장소 → 지점 [누를 곳, 걸린 시간]
  places(el, feat) {
    const m = move(), st = lastMove || {}, busy = !!st.running;
    const all = (m.places ||= []);
    const mine = all.map((pl, i) => [pl, i]).filter(([pl]) => (pl.feat || 'mfish') === feat);
    const meas = st.measuring && mine.some(([, i]) => i === st.measuring.place) ? st.measuring : null;
    const fixed = MOVE_FIXED.includes(feat);              // 정해진 장소만 쓰는 기능 (장소를 만들거나 지우지 않음)
    el.innerHTML = `
      ${meas ? `<div class="row move-measure"><span>시간 재는 중<small>캐릭터가 도착하면 F6 을 누르거나 [도착] 을 누르기</small></span>
        <span class="pos"><b data-move-timer>0.0초</b><button class="btn mini" type="button" data-move-arrive>도착</button>
        <button class="btn mini ghost" type="button" data-move-stop>멈춤</button></span></div>` : ''}
      <div class="row"><span>장소<small>멀리 갈 때는 지점을 여러 개로</small></span>
        <span class="pos">${busy && !meas ? `<b class="move-state">${esc(st.msg || '이동 중')}</b>` : ''}
        ${fixed ? '' : '<button class="btn mini" type="button" data-move-add-place>장소 추가</button>'}</span></div>` +
      (mine.length ? '' : `<div class="row"><span><small>아직 장소 없음 — [장소 추가] 로 만들기</small></span></div>`) +
      mine.map(([pl, i]) => `
      <div class="move-place">
        <div class="row"><span>${pl.key ? `<span class="move-head"><b class="move-title">${esc(pl.name)}</b>${placeTotal(pl)}</span>` :
          `<input class="move-name" data-move-name="${i}" value="${esc(pl.name || '')}" placeholder="장소 이름 (예: Captain Flarg)" maxlength="40">`}</span>
          <span class="pos"><button class="btn mini" type="button" data-move-go="${i}" ${busy ? 'disabled' : ''}>이 장소로 이동</button>
          ${pl.key ? '' : `<button class="btn mini ghost" type="button" data-move-del-place="${i}">장소 삭제</button>`}</span></div>` +
        pl.points.map((pt, j) => `
        <div class="row move-point"><span>${j + 1}번 지점${pl.feat === 'mcraft' ? (j === 0 ? ' (퀘스트 보드)' : ' (스텔라 포탈)') : ''}<small>누를 곳 ${fmtPos(pt.pos)} · 걸린 시간 <b class="${pt.time == null ? 'warn' : ''}">${fmtT(pt.time)}</b></small></span>
          <span class="pos">
            <button class="btn mini ghost" type="button" data-move-pick="${i},${j},1" ${busy ? 'disabled' : ''}>기준 장소에서 지정</button>
            <button class="btn mini ghost" type="button" data-move-pick="${i},${j},0" ${busy ? 'disabled' : ''}>바로 지정</button>
            <button class="btn mini" type="button" data-move-test="${i},${j},1" ${busy || !pt.pos ? 'disabled' : ''}>시간 재기</button>
            ${pl.points.length > 1 && pl.feat !== 'mcraft' ? `<button class="btn mini ghost" type="button" data-move-del-point="${i},${j}">삭제</button>` : ''}</span></div>`).join('') +
        (pl.feat === 'mcraft' ? '' : `
        <div class="row"><span></span><span class="pos"><button class="btn mini ghost" type="button" data-move-add-point="${i}">지점 추가</button></span></div>`) + `
      </div>`).join('');
    el.querySelector('[data-move-arrive]')?.addEventListener('click', () => api('move_arrive'));
    el.querySelector('[data-move-stop]')?.addEventListener('click', () => api('move_stop'));
    el.querySelector('[data-move-add-place]')?.addEventListener('click', () => {
      all.push({ name: '', feat, points: [{ pos: null, time: null }] }); saveMove(); rerenderMove();
    });
    el.querySelectorAll('[data-move-name]').forEach(i => i.addEventListener('input', () => {
      all[+i.dataset.moveName].name = i.value; saveMove();
    }));
    el.querySelectorAll('[data-move-del-place]').forEach(b => b.addEventListener('click', () => {
      if (!(b._armed > Date.now())) { b._armed = Date.now() + 3000; b.textContent = '한 번 더 누르면 삭제'; return; }
      all.splice(+b.dataset.moveDelPlace, 1); saveMove(); rerenderMove();
    }));
    el.querySelectorAll('[data-move-add-point]').forEach(b => b.addEventListener('click', () => {
      all[+b.dataset.moveAddPoint].points.push({ pos: null, time: null }); saveMove(); rerenderMove();
    }));
    el.querySelectorAll('[data-move-del-point]').forEach(b => b.addEventListener('click', () => {
      const [i, j] = b.dataset.moveDelPoint.split(',').map(Number);
      all[i].points.splice(j, 1); saveMove(); rerenderMove();
    }));
    el.querySelectorAll('[data-move-go]').forEach(b => b.addEventListener('click', () =>
      moveCall('move_place', { place: +b.dataset.moveGo }, '장소로 이동 시작 · 정지: F7')));
    el.querySelectorAll('[data-move-test]').forEach(b => b.addEventListener('click', async () => {
      const [i, j, fb] = b.dataset.moveTest.split(',').map(Number);
      await flushSave();
      moveCall('move_test', { place: i, point: j, from_base: !!fb },
        fb ? '기준 장소로 간 뒤 지점을 누르고 시간 재기 · 도착하면 F6' : '지점을 누르고 시간 재기 · 도착하면 F6');
    }));
    el.querySelectorAll('[data-move-pick]').forEach(b => b.addEventListener('click', async () => {
      const [i, j, fb] = b.dataset.movePick.split(',').map(Number);
      await flushSave();
      const r = await pickWith(b, fb ? '기준 장소로 가는 중… 그다음 걸어갈 곳 클릭' : '로블록스 화면에서 걸어갈 곳 클릭',
        () => api('move_pick', { place: i, point: j, from_base: !!fb }));
      if (r && r.move) { config.move = r.move; rerenderMove(); toast(`${j + 1}번 지점 저장 · 이제 [시간 재기]`); }
    }));
  },
};
// 시간 재는 중이면 0.1초마다 화면의 타이머를 올림 (poll 은 0.3초 간격이라)
setInterval(() => {
  const t = lastMove && lastMove.measuring && document.querySelector('[data-move-timer]');
  if (t) t.textContent = `${Math.max(0, Date.now() / 1000 - lastMove.measuring.since).toFixed(1)}초`;
}, 100);
// 이동 상태 (poll) — 끝나면 서버가 저장한 시간을 다시 받아 옴
function updateMove(st) {
  const was = !!(lastMove && lastMove.running), now = !!(st && st.running);
  const measChanged = !!(lastMove && lastMove.measuring) !== !!(st && st.measuring);
  lastMove = st;
  if (!document.querySelector('[data-mpos-custom]')) return;
  if (was !== now || measChanged) {
    if (was && !now) api('move_get').then(r => { if (r.move) { config.move = r.move; rerenderMove(); } });
    else rerenderMove();
    return;
  }
  document.querySelectorAll('.move-state').forEach(s => setText(s, now ? (st.msg || '이동 중') : '대기'));
}
// 위치는 서버가 이미 저장함 → 화면만 다시 그림 (준비 상태 · ▾ · 왼쪽 목록 표시도)
function mposChanged(feat) {
  renderMpos(feat);
  if (feat === 'base') renderPopSet();           // 연동이면 오토 팝핑 위치 칸도 같은 위치
  syncAllReady();
  Jump.refresh();
  setTimeout(() => Tutorial.refresh(), 0);
}
// 스나이프 탭 오토 팝핑 ↔ 공통 위치 연동 (양쪽 화면에서 같은 스위치)
function setPopLink(on) {
  pop().use_base = !!on; savePop();
  // 누른 스위치가 움직이는 걸 먼저 보여 주고 (바로 다시 그리면 스위치가 새로 생겨서 애니메이션이 안 보임) 끝나면 다시 그림
  document.querySelectorAll('#popLink, [data-mpos-link]').forEach(i => { i.checked = !!on; });
  clearTimeout(setPopLink.timer);
  setPopLink.timer = setTimeout(() => { renderMpos('base'); renderPopSet(); syncAllReady(); Tutorial.refresh(); }, 230);
  toast(on ? '스나이프 오토 팝핑이 공통 위치를 씀' : '스나이프 오토 팝핑은 따로 지정한 위치를 씀');
}
$('mposPopCopy').addEventListener('click', async () => {
  const r = await api('mpos_copy_pop');
  if (r.error) return toast(r.error);
  Object.assign(base(), r.base); mposChanged('base'); toast('오토 팝핑 위치를 가져옴');
});
$('mposPopOcr').addEventListener('click', async e => {
  const b = e.currentTarget;
  b.disabled = true;
  try {
    const r = await Promise.race([api('mpop_ocr_test'), wait(25000).then(() => ({ error: 'OCR 응답 없음 · 로그 확인' }))]);
    if (r.error) toast(r.error);
    else toast((r.text ? `이름: ${r.name || '-'} · 개수: ${r.count ?? '표시 없음(1개)'}` : '읽은 글자 없음') + ` · ${r.engine}`);
  } catch (err) { toast('OCR 오류: ' + err.message); }
  finally { b.disabled = false; }
});
$('mfishCheck').addEventListener('click', async e => {
  const b = e.currentTarget; b.disabled = true;
  try {
    const r = await api('mfish_check');
    if (r.error) return toast(r.error);
    toast([r.button && `버튼: ${r.button}`, r.bar && `릴링 바: ${r.bar}`, r.diamond && `◇: ${r.diamond}`, r.notice && `알림: ${r.notice}`, r.title && `결과창: ${r.title}`].filter(Boolean).join(' · ') || '지정된 위치 없음');
  } finally { b.disabled = false; }
});

// ---------------------------------------------------------------- 기능마다 필요한 위치 · 장소
// 매크로 설정의 [준비 상태] · 왼쪽 목록 표시 · 메인 화면 ▾ · 기능별 설정 안내 튜토리얼이 같이 씀
const placesOk = feat => {
  const mine = (move().places || []).filter(pl => pl.feat === feat);
  return mine.length > 0 && mine.every(pl => pl.points.length && pl.points.every(pt => pt.pos && pt.time != null));
};
const SELL_REQ = ['sell_fish_pos', 'first_fish_pos', 'sell_all_pos', 'confirm_sell_pos', 'shop_close_pos'];
const rowOf = (box, sel) => () => $(box)?.querySelector(sel)?.closest('.row');
// 할 일: label 이름 · feat 위치 묶음 · sec 매크로 설정의 칸 · side 왼쪽 목록 이름 · keys 지정할 칸 · optional 선택
//        cal 자동 보정을 한 번이라도 했는지 · auto 자동 보정 안내 · places 그 기능이 가는 장소 · steps 따로 하는 단계
const TASKS = {
  inv: { label: '인벤토리 위치', feat: 'base', sec: 'mf-base', side: '공통 위치', keys: [...POP_POS.map(([k]) => k), 'ocr_region'] },
  notice: { label: '알림 영역', feat: 'base', sec: 'mf-base', side: '공통 위치', keys: ['notice_region'] },   // 낚시 Cannot Fish · Auto Crafted 알림
  dialog: { label: '대화창 위치', feat: 'base', sec: 'mf-base', side: '공통 위치', keys: ['dialog_pos'] },
  moveBase: { label: '기준 장소 위치', feat: 'base', sec: 'mf-base', side: '공통 위치', keys: ['chat_pos', 'collection_pos', 'collection_close'] },
  fishWin: { label: '낚시 창 위치', feat: 'mfish', sec: 'mf-fish', side: '자동 낚시', keys: MFISH_REQ,
    manual: ['panel_region', 'reel_region', 'result_region'], cal: 'cal_win',
    auto: { target: rowOf('mposFish', '[data-fish-auto]'), title: '낚시 창 자동 보정을 해주세요',
      body: `<p>낚시 장소에서 <b>Fish</b> 버튼이 보일 때 <b>자동 보정</b>을 눌러주세요.</p>
             <p class="dim">Fish 를 직접 눌러 입질까지 재므로 끝날 때까지 손을 떼주세요. 취소는 F7.</p>` } },
  fishSell: { label: '물고기 판매 위치', feat: 'mfish', sec: 'mf-fish', side: '자동 낚시', keys: SELL_REQ, cal: 'cal_sell',
    auto: { target: rowOf('mposFish', '[data-sell-auto]'), title: '판매 자동 보정을 해주세요',
      body: `<p><b>판매 자동 보정</b>을 누른 뒤 Captain Flarg 앞에서 <b>E</b> 를 눌러주세요.</p>
             <p class="dim">실제로 팔지는 않습니다.</p>` } },
  merchChat: { label: '채팅 글자 영역', feat: 'mmerch', sec: 'mf-merchant', side: '상인 자동 구매', keys: ['chat_region'] },
  merchShop: { label: '상인 상점 위치', feat: 'mmerch', sec: 'mf-merchant', side: '상인 자동 구매', keys: [], optional: true, cal: 'cal_shop',
    auto: { target: rowOf('mposMerch', '[data-merch-auto]'), title: '상인 자동 보정을 해주세요',
      body: `<p>상인이 와 있을 때 <b>자동 보정</b>을 누른 뒤 상인 앞에서 <b>E</b> 를 눌러주세요. 사지는 않습니다.</p>
             <p class="dim">바로 위 <b>상점 위치 자동 보정</b>이 켜져 있으면 살 때마다 알아서 맞추므로 건너뛰어도 됩니다.</p>` } },
  craftRev: { label: '퀘스트 보드 Exit', sec: 'mf-craft', side: '포션 자동 제작',
    steps: [{ title: '퀘스트 보드 Exit 위치를 지정해주세요', target: rowOf('mposCraft', '[data-quest-pick]'),
      body: `<p>퀘스트 보드 앞에서 <b>E</b> 로 창을 연 뒤 <b>바로 지정</b>을 누르고 <b>Exit</b> 를 클릭해주세요.</p>`,
      ok: () => !!move().quest_exit_pos }] },
  craftShop: { label: '제작 창 위치', feat: 'mcraft', sec: 'mf-craft', side: '포션 자동 제작', optional: true, cal: 'cal_shop',
    keys: ['search_pos', 'list_region', 'shop_close_pos', 'open_recipe_pos', 'add_all_pos', 'craft_pos', 'add_close_pos'],
    auto: { target: rowOf('mposCraft', '[data-craft-auto]'), title: '제작 창 자동 보정을 해주세요',
      body: `<p><b>자동 보정</b>을 누른 뒤 Stella 앞에서 <b>F</b> 를 눌러주세요. 재료는 넣지 않습니다.</p>` } },
  matchNote: { label: '메모리 매치 창 위치', feat: 'mmatch', sec: 'mf-memory', side: '오토 메모리 매치', optional: true, cal: 'cal_note',
    keys: ['note_region', 'note_close_pos', 'note_btn_pos', 'ad_pos'],
    auto: { target: rowOf('mposMatch', '[data-match-auto]'), title: '메모리 매치 자동 보정을 해주세요',
      body: `<p><b>자동 보정</b>을 누른 뒤 메모리 매치 보드 앞에서 <b>E</b> 를 눌러주세요. Start · 광고는 누르지 않습니다.</p>` } },
  matchBoard: { label: '카드 판 위치', feat: 'mmatch', sec: 'mf-memory', side: '오토 메모리 매치', optional: true, cal: 'cal_board', keys: [],
    auto: { target: rowOf('mposMatch', '[data-board-auto]'), title: '카드 판 자동 보정을 해주세요',
      body: `<p>메모리 매치를 시작해서 <b>카드 판</b>이 떠 있을 때 <b>자동 보정</b>을 눌러주세요. 카드 20장 · 남은 기회 · Close 자리를 맞춥니다.</p>
             <p class="dim">안 해도 글자로 카드 판을 찾습니다. 선택 항목입니다.</p>` } },
};
for (const feat of ['mfish', 'mcraft', 'mmatch'])
  TASKS[feat + 'Place'] = { label: { mfish: '낚시 · 판매 장소', mcraft: '포션 제작 장소', mmatch: '메모리 매치 장소' }[feat],
    sec: FEAT_SEC[feat], side: FEAT_NAME[feat], places: feat };
// 기능마다 할 일 (순서대로 안내)
const FEAT_TASKS = { mpop: ['inv'], mitem: ['inv'], mblood: ['inv'],
  mmerch: ['inv', 'dialog', 'merchChat', 'merchShop'],
  mfish: ['fishWin', 'notice', 'moveBase', 'mfishPlace', 'fishSell', 'dialog'],
  mcraft: ['notice', 'moveBase', 'craftRev', 'mcraftPlace', 'craftShop'],
  mmatch: ['moveBase', 'mmatchPlace', 'matchNote', 'matchBoard'] };
const taskCalDone = t => !t.cal || !!featCfg(t.feat)[t.cal];
const taskKeysOk = t => {
  if (t.places) return placesOk(t.places);
  if (t.steps) return t.steps.every(q => q.ok());
  return (t.keys || []).every(k => featCfg(t.feat)[k]);
};
// 이 기능을 쓰는 데 꼭 필요한데 비어 있는 것 (이름 목록 · 선택 항목은 빼고)
function featMissing(k) {
  const out = [];
  if ((k === 'mpop' || k === 'mblood') && !(config.biome?.player || '').trim()) out.push('플레이어 이름');   // 바이옴 감지에 필요
  if (k === 'mmerch' && !Object.values(mmerch().buy || {}).some(v => v > 0)) out.push('살 아이템');
  for (const id of FEAT_TASKS[k] || []) { const t = TASKS[id]; if (!t.optional && !taskKeysOk(t)) out.push(t.label); }
  return out;
}
// '필요: 가, 나, 다' (이름 안에 · 가 있어도 쉼표로 나눠서 번역)
const missText = miss => miss.length > 3 ? `필요: ${miss.slice(0, 3).join(', ')} 외 ${miss.length - 3}개` : `필요: ${miss.join(', ')}`;
// 기능 칸 맨 위 [준비 상태]
function syncReady(k) {
  const miss = featMissing(k);
  document.querySelectorAll(`[data-ready="${k}"]`).forEach(el => {
    setText(el, miss.length ? missText(miss) : '준비 완료');
    el.classList.toggle('warn', miss.length > 0);
    el.classList.toggle('ok', !miss.length);
    el.title = miss.join(' · ');
  });
}
function syncAllReady() { FEATS.forEach(syncReady); syncFeatItems(); Quick.sync(); }
// 매크로 설정 왼쪽 목록: 기능마다 켜짐 · 꺼짐 · 설정 필요 (같으면 다시 안 씀)
function syncFeatItems() {
  for (const k of FEATS) {
    const it = document.querySelector(`[data-feat-item="${k}"]`);
    if (!it) continue;
    const on = !!featCfg(k).enabled, n = on ? featMissing(k).length : 0;
    setData(it, 'st', !on ? 'off' : n ? 'warn' : 'on');
    setText(it.querySelector('[data-feat-sub]'), !on ? '꺼짐' : n ? `켜짐 · 설정 ${n}개 필요` : '켜짐');
  }
}

// ---------------------------------------------------------------- 메인 화면 ▾ — 세부 기능을 그 자리에서 바로 켜고 끔
// 매크로: 기능 7개 (+ 전부 켜기 · 끄기) · 모드 타일의 ▾ 는 Quick.add 로 (예: 오토 스나이핑 — 팝핑할 바이옴 · 끝나고 할 일)
const Quick = (() => {
  let open = null;
  const providers = {};     // 모드: {render(panel), sync(panel), click(e) → true 면 처리함 (다시 그림), reset() (열 때), wide}
  const panel = k => document.querySelector(`[data-quick-panel="${k}"]`);
  const btnOf = k => document.querySelector(`[data-quick="${k}"]`);
  const featLine = f => { const n = featMissing(f).length; return n ? [`설정 ${n}개 필요`, true] : ['준비 완료', false]; };
  const row = (attr, name, sub, on, warn, star) => `
    <label class="quick-row"><span class="q-name"><b>${star ? '<i class="q-star">★</i>' : ''}${name}</b><small class="${warn ? 'warn' : ''}" data-q-sub>${sub}</small></span>
      <span class="switch"><input type="checkbox" ${attr} ${on ? 'checked' : ''}><i></i></span></label>`;
  function render(k) {
    const p = panel(k);
    if (k === 'macro') {
      p.innerHTML = `<div class="quick-in">
        <div class="quick-head"><b>기능 켜기 · 끄기</b><small>켜 둔 기능만 매크로를 켰을 때 동작</small></div>
        <div class="quick-list">${FEATS.map(f => { const [sub, warn] = featLine(f); return row(`data-feat="${f}"`, FEAT_NAME[f], sub, featCfg(f).enabled, warn); }).join('')}</div>
        <div class="quick-foot"><button class="btn ghost mini" type="button" data-q-all="0">전부 끄기</button>
          <button class="btn ghost mini" type="button" data-q-all="1">전부 켜기</button><span class="grow"></span>
          <button class="btn mini" type="button" data-q-open="mfeat">매크로 설정</button></div></div>`;
    } else if (providers[k]) {
      providers[k].render(p);
    }
    p.classList.toggle('wide', !!providers[k]?.wide);
    addDeco(p);                                   // 다시 그리면 ㄱ 장식도 다시
    sync();
  }
  // 열려 있으면 글자 · 스위치만 맞춤 (다시 그리면 누른 스위치의 움직임이 안 보임)
  function sync() {
    if (!open) return;
    const p = panel(open);
    if (open === 'macro') p.querySelectorAll('[data-feat]').forEach(i => {
      const f = i.dataset.feat, [sub, warn] = featLine(f), el = i.closest('.quick-row').querySelector('[data-q-sub]');
      i.checked = !!featCfg(f).enabled;
      setText(el, sub); el.classList.toggle('warn', warn);
    });
    else providers[open]?.sync?.(p);
  }
  // 창이 작으면 화면 아래로 넘치지 않게 (넘치는 만큼은 안에서 스크롤)
  function fit() {
    if (!open) return;
    const p = panel(open);
    p.style.maxHeight = '';
    const avail = innerHeight - p.getBoundingClientRect().top - 10;
    if (p.getBoundingClientRect().height > avail) p.style.maxHeight = u(Math.max(160, avail)) + 'px';
  }
  function show(k) {
    if (open === k) return hide();
    hide();
    const t = tile(k), p = panel(k);
    open = k;
    providers[k]?.reset?.();
    render(k);
    t.classList.add('quick-open');
    btnOf(k).setAttribute('aria-expanded', 'true');
    p.hidden = false;
    fit();
    requestAnimationFrame(() => p.classList.add('on'));
    setTimeout(() => Tutorial.refresh(), 0);
  }
  function hide() {
    if (!open) return;
    const k = open, t = tile(k), p = panel(k);
    open = null;
    t.classList.remove('quick-open');
    btnOf(k).setAttribute('aria-expanded', 'false');
    p.classList.remove('on');
    setTimeout(() => { if (open !== k) p.hidden = true; }, 180);
  }
  // ▾ 버튼 (모드 타일은 나중에 붙어서 위임으로)
  document.addEventListener('click', e => { const b = e.target.closest('[data-quick]'); if (b) show(b.dataset.quick); });
  addEventListener('click', e => {
    if (!open) return;
    const all = e.target.closest('[data-q-all]'), go = e.target.closest('[data-q-open]');
    if (providers[open]?.click?.(e)) {               // 모드 칸 안의 버튼 (다시 그려서 누른 버튼이 사라져도 패널은 그대로)
      render(open); fit();
      return;
    }
    if (all) return setAllFeatures(all.dataset.qAll === '1');
    if (go) {
      const key = go.dataset.qOpen, m = MENU.find(x => x.key === key);
      hide();
      if (m && curTab !== m.tab) { setTab(m.tab); setTimeout(() => openPage(key), 380); }
      else openPage(key);
      return;
    }
    if (!e.target.closest('.main-tile.quick-open')) hide();      // 바깥을 누르면 닫힘
  });
  addEventListener('keydown', e => { if (e.key === 'Escape') hide(); });
  addEventListener('resize', () => { if (open && !Tutorial.isOpen()) hide(); });
  return { show, hide, sync, isOpen: k => open === k, row, add(k, p) { providers[k] = p; } };
})();

// ---------------------------------------------------------------- 긴 칸 위쪽: 바로 가기 (묶음 이름을 누르면 그 묶음으로)
// 칸(.sec[data-jump]) 안의 묶음(.grp)과 data-jump-label 이 붙은 카드를 위에서부터
// · 묶음이 3개 이상이고 내용이 화면보다 꽤 길 때만 (짧은 칸은 바로 가기 없음)
// · 누르면 부드럽게 스크롤 (브라우저 설정과 상관없이 직접 움직임) · 표시 막대가 미끄러지듯 이동 · 도착한 묶음을 잠깐 강조
// · 칸 맨 아래에 빈 자리를 둬서 마지막 묶음들도 위쪽까지 올라옴 (마지막 전 묶음을 눌렀는데 마지막이 표시되던 문제)
const Jump = (() => {
  // 칸 안에서의 위치 (중간에 움직이는 중인 칸이 끼어 있어도 칸 기준으로 · 화면 배율과 상관없이)
  const offIn = (el, sec) => { let y = 0; while (el && el !== sec) { y += el.offsetTop; el = el.offsetParent; } return y; };
  const GAP = 14;
  function build(sec) {
    if (!sec || !sec.hasAttribute('data-jump') || !sec.classList.contains('active')) return;
    const ts = [...sec.querySelectorAll('[data-jump-label], .grp')];
    let bar = sec.querySelector(':scope > .jump'), pad = sec.querySelector(':scope > .jump-pad');
    // 내용 길이 (바로 가기 막대 · 빈 자리 빼고)
    const extra = (bar ? bar.offsetHeight : 0) + (pad ? pad.offsetHeight : 0);
    const long = sec.scrollHeight - extra > sec.clientHeight * 1.35;
    if (ts.length < 3 || !long) { bar?.remove(); pad?.remove(); return; }
    const labels = ts.map(t => t.dataset.jumpLabel || I18N.source(t.querySelector('.grp-head b')) || '');
    if (!bar) {
      bar = document.createElement('div');
      bar.className = 'jump';
      (sec.querySelector(':scope > .hint') || sec.querySelector(':scope > .sec-head')).after(bar);
      sec.addEventListener('scroll', () => {
        if (!sec._jr) sec._jr = requestAnimationFrame(() => { sec._jr = 0; if (!sec._anim) mark(sec); });
      }, { passive: true });
      // 사용자가 직접 휠 · 끌기로 움직이면 진행 중인 이동은 멈춤
      ['wheel', 'touchstart', 'mousedown'].forEach(ev => sec.addEventListener(ev, e => {
        if (sec._anim && !e.target.closest?.('.jump')) stop(sec);
      }, { passive: true }));
    }
    if (!pad) {
      pad = document.createElement('div');
      pad.className = 'jump-pad';
      sec.append(pad);
    } else if (pad !== sec.lastElementChild) sec.append(pad);
    bar._ts = ts;
    const key = labels.join('|');
    if (bar._key !== key) {
      bar._key = key;
      bar.innerHTML = '<i class="jump-ind"></i>' + labels.map((l, n) => `<button type="button" class="jump-btn" data-n="${n}">${esc(l)}</button>`).join('');
      bar._cur = -1;
    }
    // 빈 자리: 마지막 묶음이 막대 바로 아래까지 올라올 만큼만
    const last = ts[ts.length - 1];
    const bottom = sec.scrollHeight - pad.offsetHeight;
    const need = sec.clientHeight - (bottom - (offIn(last, sec) - bar.offsetHeight - GAP)) - 2;
    pad.style.height = Math.max(0, Math.round(need)) + 'px';
    mark(sec, true);
  }
  // 지금 보고 있는 묶음 표시
  function mark(sec, instant) {
    const bar = sec.querySelector(':scope > .jump');
    if (!bar || !bar._ts) return;
    const top = sec.scrollTop + bar.offsetHeight + 40;
    let cur = 0;
    bar._ts.forEach((t, n) => { if (offIn(t, sec) <= top) cur = n; });
    show(bar, cur, instant);
  }
  // 표시 막대를 그 버튼 위치로 (미끄러지듯)
  function show(bar, n, instant) {
    const btns = bar.querySelectorAll('.jump-btn'), b = btns[n], ind = bar.querySelector('.jump-ind');
    btns.forEach((x, k) => x.classList.toggle('on', k === n));
    if (!b || !ind) return;
    if (instant || bar._cur < 0) ind.classList.add('still');
    ind.style.width = b.offsetWidth + 'px';
    ind.style.height = b.offsetHeight + 'px';
    ind.style.transform = `translate(${b.offsetLeft}px, ${b.offsetTop}px)`;
    if (ind.classList.contains('still')) { void ind.offsetWidth; ind.classList.remove('still'); }
    bar._cur = n;
  }
  const ease = t => (t < .5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
  function stop(sec) {
    if (sec._anim) cancelAnimationFrame(sec._anim.raf);
    sec._anim = null;
  }
  function go(sec, y, done) {
    stop(sec);
    const from = sec.scrollTop, to = Math.max(0, Math.min(y, sec.scrollHeight - sec.clientHeight)), dist = to - from;
    if (Math.abs(dist) < 2) { sec.scrollTop = to; done?.(); return; }
    const dur = Math.min(620, Math.max(280, 220 + Math.abs(dist) * 0.35)), t0 = performance.now();
    const a = sec._anim = { raf: 0 };
    const step = now => {
      if (sec._anim !== a) return;
      const k = Math.min(1, (now - t0) / dur);
      sec.scrollTop = from + dist * ease(k);
      if (k < 1) a.raf = requestAnimationFrame(step);
      else { sec._anim = null; done?.(); }
    };
    a.raf = requestAnimationFrame(step);
  }
  document.addEventListener('click', e => {
    const b = e.target.closest('.jump-btn');
    if (!b) return;
    const bar = b.parentElement, sec = bar.parentElement, n = +b.dataset.n, t = bar._ts?.[n];
    if (!t) return;
    show(bar, n);
    go(sec, offIn(t, sec) - bar.offsetHeight - GAP, () => {
      mark(sec);
      t.classList.remove('jump-hit'); void t.offsetWidth; t.classList.add('jump-hit');
      clearTimeout(t._hit); t._hit = setTimeout(() => t.classList.remove('jump-hit'), 1000);
    });
  });
  // 칸 안이 다시 그려졌을 때 (위치 지정 · 장소 추가 등) · 창 크기가 바뀌었을 때
  const refresh = () => document.querySelectorAll('.sec[data-jump].active').forEach(build);
  let rt = 0;
  addEventListener('resize', () => { clearTimeout(rt); rt = setTimeout(refresh, 120); });
  return { build, refresh };
})();

// ---------------------------------------------------------------- 통계 (Acrux 탭) — 이번 실행 (Acrux 를 켠 뒤부터) · 올타임 (지금까지 전부)
let lastStats = null, statsKey = '';
const STAT_GROUPS = [
  ['자동 낚시', [['fish_success', '성공'], ['fish_junk', '쓰레기'], ['fish_fail', '실패'], ['fish_full', '인벤토리 가득']]],
  ['오토 아이템 사용', [['item_strange', 'Strange Controller'], ['item_randomizer', 'Biome Randomizer']]],
  ['상인 자동 구매', [['merch_bought', '구매한 아이템']]],
  ['포션 자동 제작', [['crafted', '재료 채운 횟수']]],
  ['오토 메모리 매치', [['match_played', '게임 수'], ['match_ads', '본 광고']]],
  ['블러드 레인 자동 생성', [['blood_used', 'Cursed Rune Fragment']]],
];
const fmtNum = n => Number(n || 0).toLocaleString('en-US');
const fmtDay = t => { const d = new Date(t * 1000), p = n => String(n).padStart(2, '0'); return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`; };
function renderStats(force) {
  const st = lastStats || {};
  const key = JSON.stringify([st.session, st.all, st.since, st.started]);
  if (!force && key === statsKey) return;
  statsKey = key;
  for (const [id, part] of [['msSession', 'session'], ['msAll', 'all']]) {
    const v = st[part] || {};
    // 기능마다 한 줄: 왼쪽 이름 · 오른쪽 숫자 칸
    $(id).innerHTML = STAT_GROUPS.map(([name, items]) => `
      <div class="stat-row"><b class="stat-name">${name}</b>
        <div class="stat-grid">${items.map(([k, label]) => `<div class="stat${v[k] ? '' : ' zero'}"><b data-notr>${fmtNum(v[k])}</b><span>${label}</span></div>`).join('')}</div></div>`).join('');
  }
  setText($('msSince'), st.started ? `${fmtDay(st.started)}부터` : '');
  setText($('msAllSince'), st.since ? `${fmtDay(st.since)}부터` : '아직 기록 없음');
}

// ---------------------------------------------------------------- 업데이트 로그 (GitHub 릴리스 설명)
// 한국어는 '업데이트' 부분, 그 외 언어는 영어 'Update' 부분을 보여줌 (노트는 한국어 + 영어로만 씀)
let changelog = null;
function mdList(md) {
  // "- 항목" 목록 (들여쓰기 2칸 = 한 단계) · **굵게** 만 지원
  const inline = t => esc(t).replace(/\*\*(.+?)\*\*/g, '<b>$1</b>');
  const root = { kids: [] }, stack = [root];
  for (const line of md.split('\n')) {
    const m = line.match(/^(\s*)- (.*)$/);
    if (!m) continue;
    const d = Math.min(Math.floor(m[1].length / 2) + 1, stack.length);
    stack.length = d;
    const node = { text: m[2], kids: [] };
    stack[d - 1].kids.push(node);
    stack.push(node);
  }
  const ul = kids => kids.length ? '<ul>' + kids.map(k => `<li>${inline(k.text)}${ul(k.kids)}</li>`).join('') + '</ul>' : '';
  return ul(root.kids);
}
function renderChangelog() {
  const box = $('acLog'), ko = I18N.lang === 'ko';
  if (!changelog) { box.innerHTML = `<div class="empty">${ko ? 'GitHub 에서 불러오는 중…' : 'Loading from GitHub…'}</div>`; return; }
  if (changelog.error) {
    box.innerHTML = `<div class="empty">${ko ? '업데이트 로그를 못 불러옴 · 인터넷 연결 확인' : 'Could not load the update log · check your internet connection'}
      <button class="btn mini" type="button" id="clRetry">${ko ? '다시 불러오기' : 'Retry'}</button></div>`;
    $('clRetry').addEventListener('click', () => loadChangelog(true));
    return;
  }
  if (!changelog.entries.length) { box.innerHTML = `<div class="empty">${ko ? '업데이트 로그 없음' : 'No update log'}</div>`; return; }
  box.innerHTML = changelog.entries.map(e => `
    <div class="card cl-item${e.version === changelog.current ? ' now' : ''}">
      <div class="cl-head"><b>V${esc(e.version)}</b>${e.version === changelog.current ? `<span class="cl-now">${ko ? '지금 버전' : 'Current'}</span>` : ''}
        <span class="grow"></span><span class="cl-date">${esc(e.date)}</span></div>
      <div class="cl-body">${mdList((ko ? e.ko : e.en) || e.ko || e.en)}</div>
    </div>`).join('');
}
// 업데이트 로그 화면을 열 때마다 (서버가 10분 동안은 받아둔 걸 줌)
async function loadChangelog(refresh = false) {
  if (!changelog || changelog.error) { changelog = null; renderChangelog(); }
  try { changelog = await api('changelog', refresh ? { refresh: true } : {}); }
  catch (err) { changelog = { error: err.message }; }
  renderChangelog();
}
I18N.onChange(() => renderChangelog());


async function refreshOcrInfo() {
  $('acOcrNow').textContent = '확인 중…';
  try {
    const r = await api('ocr_info');
    $('acOcrNow').textContent = r.engine + (r.rapid ? '' : ' (RapidOCR 없음)');
  } catch { $('acOcrNow').textContent = '-'; }
}
function fillAcrux() {
  $('acOcr').value = config.ocr_engine || 'auto';
  const sel = $('acLang');
  sel.innerHTML = Object.entries(I18N.LANGS).map(([k, n]) => `<option value="${k}">${n}</option>`).join('');
  sel.value = I18N.lang;
  I18N.onChange(l => { sel.value = l; });
  $('acVer').textContent = $('verText').textContent;
  $('acOnline').checked = config.online_share !== false;
}
$('acOcr').addEventListener('change', e => {
  config.ocr_engine = e.target.value;
  queueSave({ ocr_engine: e.target.value });
  setTimeout(refreshOcrInfo, 600);             // 저장된 뒤 다시 확인
});
$('acLang').addEventListener('change', e => setLang(e.target.value));
// 사용자 수 (지금 Acrux 를 켜 둔 사람) · 집계 참여 스위치
function updateOnline(n) {
  setText($('nowActiveN'), n == null ? '--' : String(n));
  $('nowActive').classList.toggle('live', n != null);
}
$('acOnline').addEventListener('change', e => { config.online_share = e.target.checked; queueSave({ online_share: e.target.checked }); });
$('acFolder').addEventListener('click', async () => {
  try { await api('open_folder'); } catch { toast('폴더를 열 수 없음'); }
});

// ---------------------------------------------------------------- 내 서버로 복귀 (복구 · 게임 입장 화면 — 모드 화면에도 [data-ret-dot])
const ret = () => (config.ret ||= { ps_link: '' });
const saveRet = () => queueSave({ ret: JSON.parse(JSON.stringify(ret())) });
function fillRet() {
  $('retLink').value = ret().ps_link || '';
  $('retStartWait').value = ret().start_wait ?? 7.5;
}
$('retStartWait').addEventListener('input', e => {
  const n = parseFloat(e.target.value);
  if (Number.isFinite(n) && n >= 0) { ret().start_wait = n; saveRet(); }
});
$('retLink').addEventListener('input', e => { ret().ps_link = e.target.value.trim(); saveRet(); });
$('retFromBiome').addEventListener('click', () => {
  const l = (config.biome?.ps_link || '').trim();
  if (!l) return toast('바이옴 매크로 설정에 브섭 링크가 없음');
  ret().ps_link = l; fillRet(); saveRet(); toast('브섭 링크 가져옴'); Tutorial.refresh();
});
$('retTest').addEventListener('click', async () => {
  const r = await api('ret_test');
  toast(r.error || '로블록스 종료 → 1초 → 내 서버 접속 → Play');
});
function updateRet(st) {
  st = Mods.pipe('ret', st);                       // 모드가 상태를 덧붙임 (예: 커스텀 매크로 실행 중)
  const run = !!(st && st.running);
  document.querySelectorAll('[data-ret-dot]').forEach(d => setData(d, 's', run ? 'flux' : ''));
  document.querySelectorAll('[data-ret-state]').forEach(t => setText(t, run ? (st.msg || '복귀 중') : '대기'));
  $('retTest').disabled = run;
}

// ---------------------------------------------------------------- 직접 만드는 매크로 (동작 목록 편집기)
// 매크로 복귀 → 커스텀 매크로 (스나이핑이 끝나고 내 서버에 들어가면 실행)
const STEP_DEFS = {
  key:    { label: '키 입력',           make: () => ({ key: 'e', count: 1, hold: 40, gap: 100 }) },
  combo:  { label: '키 조합',           make: () => ({ keys: 'ctrl+l', count: 1, hold: 40, gap: 100 }) },
  click:  { label: '마우스 클릭',       make: () => ({ x: null, y: null, button: 'left', count: 1, gap: 150 }) },
  wait:   { label: '대기',              make: () => ({ ms: 1000 }) },
  text:   { label: '글자 입력',         make: () => ({ text: '', enter: false, gap: 20 }) },
  scroll: { label: '스크롤',            make: () => ({ amount: -3, gap: 100 }) },
  run:    { label: '프로그램 실행',     make: () => ({ path: '', args: '', wait_ms: 1000 }) },
  kill:   { label: '프로그램 강제 종료', make: () => ({ exe: '', gap: 300 }) },
  focus:  { label: '창 맨 앞으로',      make: () => ({ target: 'roblox', title: '', exe: '', wait_ms: 3000 }) },
};
let KEY_LIST = [];

function field(label, input, wide) {
  const w = document.createElement('label');
  w.className = 'f' + (wide ? ' wide' : '');
  w.innerHTML = '<span></span>';
  w.firstChild.textContent = label;
  w.appendChild(input);
  return w;
}
function btn(label, fn) {
  const b = document.createElement('button');
  b.className = 'btn mini ghost'; b.type = 'button'; b.textContent = label;
  b.addEventListener('click', () => fn(b));
  return b;
}

// 시간 값: 화면에선 초 (0.1초), 저장은 ms (100) — 키 입력 등은 컴퓨터에 그대로 보냄 (로블록스 전용 아님)
function makeStepEditor({ list, save, addBar, listEl, emptyEl, types }) {
  const num = (obj, key, min = 0) => {
    const i = document.createElement('input');
    i.type = 'number'; i.min = min; i.value = obj[key] ?? '';
    i.addEventListener('input', () => { const n = parseInt(i.value, 10); if (Number.isFinite(n)) { obj[key] = n; save(); } });
    return i;
  };
  const sec = (obj, key) => {
    const i = document.createElement('input');
    i.type = 'number'; i.min = 0; i.step = 0.05;
    i.value = obj[key] == null ? '' : +(obj[key] / 1000).toFixed(3);
    i.addEventListener('input', () => { const n = parseFloat(i.value); if (Number.isFinite(n) && n >= 0) { obj[key] = Math.round(n * 1000); save(); } });
    return i;
  };
  const text = (obj, key, ph = '') => {
    const i = document.createElement('input');
    i.value = obj[key] ?? ''; i.placeholder = ph; i.spellcheck = false;
    i.addEventListener('input', () => { obj[key] = i.value; save(); });
    return i;
  };
  const select = (obj, key, options, onChange) => {
    const s = document.createElement('select');
    for (const [v, t] of options) { const o = document.createElement('option'); o.value = v; o.textContent = t; s.appendChild(o); }
    s.value = obj[key];
    s.addEventListener('change', () => { obj[key] = s.value; save(); onChange && onChange(); });
    return s;
  };
  const check = (obj, key, label) => {
    const w = document.createElement('label');
    w.className = 'f chk';
    w.innerHTML = `<input type="checkbox" ${obj[key] ? 'checked' : ''}><span></span>`;
    w.lastChild.textContent = label;
    w.firstChild.addEventListener('change', e => { obj[key] = e.target.checked; save(); });
    return w;
  };

  addBar.dataset.tctx = 'step';
  function renderAdd() {
    addBar.innerHTML = '';
    for (const type of types) {
      const d = STEP_DEFS[type];
      addBar.appendChild(btn('+ ' + d.label, () => {
        const s = { type, enabled: true, ...d.make() };
        list().push(s);
        save(); render();
        listEl.lastElementChild?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }));
    }
  }

  function render() {
    const L = list();
    listEl.innerHTML = '';
    emptyEl.style.display = L.length ? 'none' : '';
    L.forEach((s, i) => {
      const card = document.createElement('div');
      card.className = 'step' + (s.enabled === false ? ' off' : '');
      card.dataset.i = i;
      const head = document.createElement('div');
      head.className = 'step-head';
      head.dataset.tctx = 'step';
      head.innerHTML = `<span class="grip" title="드래그해서 순서 바꾸기"><i></i><i></i></span><span class="num">${i + 1}</span><b>${STEP_DEFS[s.type]?.label || s.type}</b>
        <label class="switch small"><input type="checkbox" ${s.enabled === false ? '' : 'checked'}><i></i></label>
        <span class="grow"></span>
        <button class="ib" data-a="up" title="위로">↑</button><button class="ib" data-a="down" title="아래로">↓</button>
        <button class="ib" data-a="dup" title="복제">⧉</button><button class="ib del" data-a="del" title="삭제">${icon('x')}</button>`;
      head.querySelector('.grip').addEventListener('pointerdown', e => dragStep(e, card, i));
      head.querySelector('input').addEventListener('change', e => { s.enabled = e.target.checked; card.classList.toggle('off', !s.enabled); save(); });
      head.querySelectorAll('.ib').forEach(b => b.addEventListener('click', () => {
        const a = b.dataset.a;
        if (a === 'up' && i > 0) [L[i - 1], L[i]] = [L[i], L[i - 1]];
        else if (a === 'down' && i < L.length - 1) [L[i + 1], L[i]] = [L[i], L[i + 1]];
        else if (a === 'dup') L.splice(i + 1, 0, JSON.parse(JSON.stringify(s)));
        else if (a === 'del') L.splice(i, 1);
        else return;
        save(); render();
      }));
      const body = document.createElement('div');
      body.className = 'step-body';
      const add = (...els) => els.forEach(e => body.appendChild(e));
      switch (s.type) {
        case 'key':
          add(field('키', select(s, 'key', KEY_LIST.map(k => [k, k.toUpperCase()]))),
              field('횟수', num(s, 'count', 1)), field('누르는 시간 (초)', sec(s, 'hold')),
              field('간격 (초)', sec(s, 'gap')));
          break;
        case 'combo': {
          const inp = text(s, 'keys', '예: ctrl+l / ctrl+shift+esc / alt+f4');
          inp.addEventListener('change', async () => {
            const r = await api('combo_check', { keys: inp.value });
            inp.classList.toggle('bad', !!r.error);
            toast(r.error ? r.error : '키 조합: ' + r.keys.map(k => k.toUpperCase()).join(' + '));
          });
          add(field('키 조합 (+ 로 연결)', inp, true), field('횟수', num(s, 'count', 1)),
              field('누르는 시간 (초)', sec(s, 'hold')), field('간격 (초)', sec(s, 'gap')));
          break;
        }
        case 'click': {
          const box = document.createElement('div');
          box.className = 'pos';
          const val = document.createElement('code');
          const show = () => { val.textContent = s.x == null ? '지정 안 됨' : `${pct(s.x)}, ${pct(s.y)}`; val.classList.toggle('unset', s.x == null); };
          show();
          box.append(val, btn('위치 지정', async b => {
            const r = await pickWith(b, '로블록스 화면에서 클릭', () => api('pick_point_overlay'));
            if (r) { s.x = r.x; s.y = r.y; show(); save(); toast('위치 저장'); }
          }));
          add(field('위치 (로블록스 창 기준)', box), field('버튼', select(s, 'button', [['left', '왼쪽'], ['right', '오른쪽']])),
              field('횟수', num(s, 'count', 1)), field('간격 (초)', sec(s, 'gap')));
          break;
        }
        case 'wait':
          add(field('시간 (초)', sec(s, 'ms')));
          break;
        case 'text':
          add(field('글자', text(s, 'text', '입력할 글자'), true), field('글자 간격 (초)', sec(s, 'gap')),
              check(s, 'enter', '입력 후 엔터'));
          break;
        case 'scroll':
          add(field('양 (음수 = 아래)', num(s, 'amount', -100)), field('간격 (초)', sec(s, 'gap')));
          break;
        case 'run': {
          const path = text(s, 'path', '예: C:\\Program Files\\...\\program.exe');
          path.style.flex = '1';
          const box = document.createElement('div');
          box.className = 'pos';
          box.append(path, btn('찾아보기', async b => {
            const r = await pickWith(b, '파일 고르는 중…', () => api('pick_file'));
            if (r) { s.path = r.path; path.value = r.path; save(); }
          }));
          add(field('프로그램 (exe · 바로가기 · bat 등)', box, true), field('실행 옵션 (선택)', text(s, 'args', '예: --minimized')),
              field('실행 후 대기 (초)', sec(s, 'wait_ms')));
          break;
        }
        case 'kill': {
          const exe = text(s, 'exe', '예: Discord.exe, Spotify.exe');
          const pickSel = windowPicker(w => { s.exe = w.exe; exe.value = w.exe; save(); toast('프로그램 선택: ' + w.exe); }, true);
          add(field('종료할 프로그램 (여러 개는 , 로 구분)', exe, true), field('열린 프로그램', pickSel), field('종료 후 대기 (초)', sec(s, 'gap')));
          break;
        }
        case 'focus': {
          const other = document.createElement('div');
          other.className = 'step-body'; other.style.padding = '0'; other.style.flexBasis = '100%';
          const title = text(s, 'title', '예: Discord / Chrome');
          const exe = text(s, 'exe', '예: Discord.exe');
          const pickSel = windowPicker(w => {
            s.title = w.title; s.exe = w.exe; title.value = w.title; exe.value = w.exe; save();
            toast('창 선택: ' + (w.exe || w.title));
          });
          other.append(field('열린 창', pickSel, true), field('창 제목에 들어간 글자', title, true),
                       field('프로그램 이름 (선택)', exe), field('창 기다리기 (초)', sec(s, 'wait_ms')));
          const sync = () => { other.style.display = s.target === 'roblox' ? 'none' : ''; };
          add(field('맨 앞으로 가져올 창', select(s, 'target', [['roblox', '로블록스'], ['window', '다른 창 (프로그램)']], sync)), other);
          sync();
          break;
        }
      }
      card.append(head, body);
      listEl.appendChild(card);
    });
  }

  // 왼쪽 II 를 잡고 끌어서 순서 바꾸기 (다른 카드는 비켜 주고, 놓으면 그 자리로)
  function dragStep(e, card, from) {
    if (e.button !== 0) return;
    e.preventDefault();
    const cards = [...listEl.children];
    const rects = cards.map(c => c.getBoundingClientRect());
    const step = rects.length > 1 ? rects[1].top - rects[0].top : rects[0].height;
    const shift = rects[from].height + (rects.length > 1 ? rects[1].top - rects[0].bottom : 8);
    const sc = listEl.closest('.sec');
    const y0 = e.clientY, s0 = sc ? sc.scrollTop : 0;
    let to = from, lastY = y0, timer = null;
    card.classList.add('dragging');
    listEl.classList.add('sorting');
    const update = () => {
      const dy = lastY - y0 + (sc ? sc.scrollTop - s0 : 0);
      card.style.transform = `translateY(${u(dy)}px)`;
      const mid = rects[from].top + rects[from].height / 2 + dy;
      to = from;
      rects.forEach((r, k) => {
        const m = r.top + r.height / 2;
        if (k < from && mid < m) to = Math.min(to, k);
        if (k > from && mid > m) to = Math.max(to, k);
      });
      cards.forEach((c, k) => {
        if (k === from) return;
        const sft = from < to && k > from && k <= to ? -shift : to < from && k >= to && k < from ? shift : 0;
        c.style.transform = sft ? `translateY(${u(sft)}px)` : '';
      });
    };
    // 목록 위·아래 끝 근처로 끌면 자동으로 스크롤
    const edge = () => {
      if (!sc) return;
      const r = sc.getBoundingClientRect(), m = 48 * Z;
      const v = lastY < r.top + m ? -10 : lastY > r.bottom - m ? 10 : 0;
      if (v) { sc.scrollTop += v; update(); }
    };
    const move = ev => { lastY = ev.clientY; update(); };
    const up = () => {
      removeEventListener('pointermove', move);
      removeEventListener('pointerup', up);
      removeEventListener('pointercancel', up);
      clearInterval(timer);
      listEl.classList.remove('sorting');
      const L = list();
      if (to !== from) {
        L.splice(to, 0, L.splice(from, 1)[0]);
        save();
      }
      render();
    };
    addEventListener('pointermove', move);
    addEventListener('pointerup', up);
    addEventListener('pointercancel', up);
    timer = setInterval(edge, 30);
  }

  let last = null;
  function setRunning(st) {
    const running = st && st.running;
    const key = running ? st.step : -1;
    if (key === last) return;
    last = key;
    listEl.querySelectorAll('.step').forEach(c => c.classList.toggle('now', running && +c.dataset.i === st.step));
  }
  return { renderAdd, render, setRunning };
}

// 열린 창 / 프로그램 고르기 (byExe: 프로그램 이름 기준으로 중복 제거)
function windowPicker(onPick, byExe = false) {
  const sel = document.createElement('select');
  const blank = byExe ? '열린 프로그램에서 고르기…' : '열린 창에서 고르기…';
  sel.innerHTML = `<option value="">${blank}</option>`;
  sel.addEventListener('focus', async () => {
    const r = await api('list_windows');
    let ws = r.windows || [];
    if (byExe) ws = ws.filter((w, n) => w.exe && ws.findIndex(x => x.exe === w.exe) === n);
    sel._list = ws;
    sel.innerHTML = `<option value="">${blank}</option>` + ws.map((w, n) =>
      `<option value="${n}">${byExe ? esc(w.exe) + ' — ' + esc(w.title) : esc(w.title) + (w.exe ? ' — ' + esc(w.exe) : '')}</option>`).join('');
  });
  sel.addEventListener('change', () => { const w = (sel._list || [])[+sel.value]; if (w) onPick(w); sel.value = ''; });
  return sel;
}

// ---------------------------------------------------------------- 바이옴 매크로
const bio = () => (config.biome ||= { enabled: false, player: '', ps_link: '', webhooks: ['', ''], everyone: {}, roles: {} });
// 덜 희귀한 것 → 희귀한 것 (멘션 설정에는 거꾸로: 희귀한 것부터)
const BIOME_LIST = ['WINDY', 'SNOWY', 'RAINY', 'SANDSTORM', 'HELL', 'STARFALL', 'HEAVEN', 'CORRUPTION', 'NULL',
                    'GLITCHED', 'DREAMSPACE', 'CYBERSPACE', 'LOSTFOREST', 'SINGULARITY', 'RADIANTFOREST',
                    'PUMPKINMOON', 'GRAVEYARD', 'BLOODRAIN'];      // 끝의 셋: 할로윈 이벤트
const saveBio = () => queueSave({ biome: JSON.parse(JSON.stringify(bio())) });
const RARE = ['GLITCHED', 'DREAMSPACE', 'CYBERSPACE'];

function fillBiome() {
  const b = bio();
  $('bioEnabled').checked = !!b.enabled;
  $('bioPlayer').value = b.player || '';
  $('bioPs').value = b.ps_link || '';
  $('bioHook1').value = (b.webhooks || [])[0] || '';
  $('bioHook2').value = (b.webhooks || [])[1] || '';
  syncBioToggles();
  renderMentions();
}

// 메인 카드 스위치와 화면 위쪽 스위치는 같은 설정
function syncBioToggles() {
  const on = !!bio().enabled;
  $('bioEnabled').checked = on;
  document.querySelectorAll('[data-toggle="biome"]').forEach(i => { i.checked = on; });
  if (config) syncMainTiles();
}
function setBioEnabled(on) {
  if (on && !bio().player) {
    syncBioToggles();                       // 플레이어 이름이 없으면 켜지 않고 설정 튜토리얼로
    toast('플레이어 이름을 먼저 입력해주세요');
    if (!Tutorial.isOpen()) Tutorial.start('biome');
    return;
  }
  bio().enabled = on; syncBioToggles(); saveBio();
  toast('바이옴 매크로 ' + (on ? '켜짐' : '꺼짐'));
}

function renderMentions() {
  const box = $('mentionList');
  box.innerHTML = '';
  const b = bio();
  b.roles ||= {}; b.everyone ||= {};
  // 레어 바이옴 먼저, 나머지는 역순 (희귀한 것부터)
  const order = [...RARE].reverse().concat(BIOME_LIST.filter(n => !RARE.includes(n)).reverse());
  for (const name of order) {
    const rare = RARE.includes(name);
    const row = document.createElement('div');
    row.className = 'mention-row' + (rare ? ' rare' : '');
    row.innerHTML = `<b></b><input placeholder="역할 ID (선택)" inputmode="numeric" spellcheck="false">` +
      (rare ? `<label class="switch"><input type="checkbox"><i></i>@everyone</label>` : '<span></span>');
    row.querySelector('b').textContent = bioName(name);
    const inp = row.querySelector('input:not([type=checkbox])');
    inp.value = b.roles[name] || '';
    inp.addEventListener('input', () => {
      const v = inp.value.replace(/[^0-9]/g, '');
      if (v !== inp.value) inp.value = v;
      if (v) b.roles[name] = v; else delete b.roles[name];
      saveBio();
    });
    if (rare) {
      const cb = row.querySelector('input[type=checkbox]');
      cb.checked = b.everyone[name] !== false;
      cb.addEventListener('change', () => { b.everyone[name] = cb.checked; saveBio(); });
    }
    box.appendChild(row);
  }
}
$('bioEnabled').addEventListener('change', e => setBioEnabled(e.target.checked));
document.addEventListener('change', e => { if (e.target.matches('[data-toggle="biome"]')) setBioEnabled(e.target.checked); });
$('bioPlayer').addEventListener('input', e => { bio().player = e.target.value.trim(); saveBio(); syncAllReady(); });
$('bioPs').addEventListener('input', e => { bio().ps_link = e.target.value.trim(); saveBio(); });
for (const [id, n] of [['bioHook1', 0], ['bioHook2', 1]]) {
  $(id).addEventListener('input', e => {
    const w = bio().webhooks = [...(bio().webhooks || ['', ''])];
    w[n] = e.target.value.trim(); saveBio();
  });
}
$('bioTest').addEventListener('click', async () => {
  const btn = $('bioTest');
  btn.disabled = true; btn.textContent = '전송 중…';
  await wait(400);                                   // 방금 입력한 주소가 저장될 시간
  try {
    const r = await api('webhook_test');
    toast(r.error || (r.queued ? '테스트 전송 대기열에 추가됨' : r.ok === r.total ? `전송 완료 (${r.ok}/${r.total})` : `일부 실패 (${r.ok}/${r.total}) · ${r.errors[0] || ''}`));
  } finally { btn.disabled = false; btn.textContent = '테스트 전송'; }
});

let lastBioHist = '';
function updateBiome(st) {
  if (!st) return;
  const cur = st.current;
  setText($('biomeNow'), cur ? `바이옴: ${bioName(cur)}` : '바이옴 인식 안 됨');
  setData($('biomeDot'), 's', cur ? (RARE.includes(cur) ? 'rare' : 'flux') : '');
  setText($('bioStatus'), st.owner === 'noname' ? '플레이어 이름 입력 필요 · 이름이 없으면 감지 안 함'
    : !st.log ? '로블록스 로그 파일 없음 · 로블록스를 한 번 실행하면 생김'
    : st.owner === 'other' ? '닉네임 불일치 · 게임 속 실제 닉네임과 입력한 이름이 다름'
    : st.owner !== 'ok' ? '계정 확인 중 · 게임에 들어가면 닉네임 확인'
    : cur ? `인식 중 · 현재 바이옴 ${bioName(cur)}` : '로그 확인 중 · 게임에 들어가면 인식');
  const key = JSON.stringify(st.history);
  if (key !== lastBioHist) {
    lastBioHist = key;
    const box = $('bioHist');
    box.innerHTML = '';
    $('bioHistCount').textContent = st.history.length ? `최근 ${st.history.length}개` : '';
    if (!st.history.length) box.innerHTML = '<span class="dim-line">기록 없음</span>';
    for (const [t, name] of [...st.history].reverse()) {
      const row = document.createElement('div');
      row.className = 'bio-row' + (RARE.includes(name) ? ' rare' : '');
      row.innerHTML = '<span class="t"></span><b></b>';
      row.children[0].textContent = fmtTime(t);
      row.children[1].textContent = bioName(name);
      box.appendChild(row);
    }
  }
}

// ---------------------------------------------------------------- 튜토리얼
// 안내 단계는 [다음]으로, 조작 단계는 사용자가 강조된 곳을 직접 눌러야 넘어감
// 순서 (자동으로 이어질 때): 바이옴 매크로 → ① 링크 감지 → ② 접속 → 게임 입장 · ③ 오토 팝핑 → ④ 복귀 → 매크로
const Tutorial = (() => {
  const el = $('tut'), card = $('tutCard'), spot = $('tutSpot');
  let i = -1, focusEl = null, moved = false, chain = false, skipCount = 0;
  const pageOf = key => () => $('page-' + key);
  const biomePage = pageOf('biome'), mfeatPage = pageOf('mfeat');
  const sectionIs = (sec, page) => page?.querySelector('.side .item.active')?.dataset.sec === sec;
  const onSec = key => sec => current === key && sectionIs(sec, $('page-' + key));
  const biomeSec = onSec('biome'), mfSec = onSec('mfeat');
  const cardOf = key => () => document.querySelector(`.menu-card[data-key="${key}"]`);
  const doneList = () => config.tutorials_done || [];
  const needsBiome = () => !(config.biome?.player) && !doneList().includes('biome');

  // 튜토리얼 목록 — 새 튜토리얼은 여기에 추가 (id 는 완료 기록용)
  // 단계 옵션: target = 강조할 곳 · done = 끝났는지 (조작 단계) · needs = 이 단계가 끝나 있어야 함 (아니면 거기로 되돌아감)
  //           input = 강조된 칸에 입력 후 [다음] · requires = 조건을 채워야 [다음] 가능
  //           optional = [이 단계 건너뛰기] 표시 · skipTo = 건너뛸 때 갈 단계 · later = 첫 안내에서 [나중에] · task = 단계 수 대신 보일 글
  // build: 시작할 때마다 단계를 새로 만듦 (지금 설정에 맞춰 필요한 것만)
  const TUTORIALS = [];
  // 받침 있으면 '을', 없으면 '를' (영어는 '를')
  const eul = w => { const c = w.charCodeAt(w.length - 1); return c >= 0xAC00 && c <= 0xD7A3 && (c - 0xAC00) % 28 ? '을' : '를'; };
  const secStep = (page, sec, label, extra = {}) => ({
    title: `${label}${eul(label)} 눌러주세요`,
    body: `<p>왼쪽 목록에서 강조된 <b>${label}</b>${eul(label)} 눌러주세요.</p>`,
    target: () => page().querySelector(`.side .item[data-sec="${sec}"]`), ...extra });
  const cardStep = (key, label, extra = {}) => ({
    title: `${label}${eul(label)} 눌러주세요`,
    body: `<p>강조된 <b>${label}</b> 버튼을 직접 눌러주세요.</p>`,
    target: cardOf(key), done: () => current === key, ...extra });
  const backStep = (key, extra = {}) => ({
    title: '뒤로 가주세요',
    body: `<p>여기 설정이 끝났습니다. 왼쪽 위 강조된 <b>뒤로</b>를 눌러 메인 화면으로 돌아가주세요.</p>`,
    target: () => $('page-' + key).querySelector('[data-back]'), done: () => current !== key, ...extra });
  const pickRow = sel => () => document.querySelector(sel)?.closest('.row');

  // ---- 바이옴 매크로 설정 — 플레이어 이름은 필수, 브섭 링크·웹후크는 선택 (그 단계만 건너뛰기 가능)
  const BIOME_STEPS = [
    { title: '바이옴 매크로 설정이 필요합니다',
      body: `<p>로블록스 로그로 현재 바이옴을 인식하고, 바이옴이 바뀌면 <b>디스코드 웹후크</b>로 알림을 보냅니다.</p>
             <p><b>플레이어 이름</b>은 꼭 입력해야 하며, <b>브섭 링크</b>와 <b>웹후크</b>는 선택입니다.</p>`,
      later: true },
    { id: 'card', ...cardStep('biome', '바이옴 매크로 설정') },
    { id: 'side', ...secStep(biomePage, 'bio-basic', '기본 설정'), done: () => biomeSec('bio-basic'), needs: 'card' },
    { title: '플레이어 이름을 입력해주세요',
      body: `<p>로블록스 <b>실제 닉네임</b>을 입력해주세요. 디스플레이 이름이 아닌 <b>@ 뒤의 이름</b>입니다.</p>
             <p class="dim">게임 속 닉네임과 같아야 바이옴을 감지합니다. 필수 항목입니다.</p>`,
      target: () => $('bioPlayer').closest('.row'), input: true, requires: () => !!bio().player, needs: 'side' },
    { title: '브섭 링크를 입력해주세요',
      body: `<p>바이옴 알림에 함께 표시할 <b>비공개 서버 링크</b>를 붙여넣어주세요.</p>
             <p class="dim">선택 항목입니다. 필요 없으면 이 단계를 건너뛰어주세요.</p>`,
      target: () => $('bioPs').closest('.row'), input: true, optional: true, needs: 'side' },
    { id: 'hookSide', ...secStep(biomePage, 'bio-hooks', '디스코드 웹후크'),
      body: `<p>알림을 디스코드로 받으려면 왼쪽 목록에서 강조된 <b>디스코드 웹후크</b>를 눌러주세요.</p>
             <p class="dim">선택 항목입니다. 알림이 필요 없으면 이 단계를 건너뛰어주세요.</p>`,
      done: () => biomeSec('bio-hooks'), optional: true, skipTo: 'done', needs: 'card' },
    { title: '웹후크 주소를 입력해주세요',
      body: `<ol><li>알림을 받을 디스코드 채널의 <b>톱니바퀴 (채널 편집)</b>를 눌러주세요.</li>
             <li><b>연동 → 웹후크 → 새 웹후크</b>를 누른 뒤 <b>웹후크 URL 복사</b>를 눌러주세요.</li>
             <li>복사한 주소를 <b>웹후크 1</b> 칸에 붙여넣어주세요.</li></ol>
             <p class="dim">튜토리얼이 끝난 뒤 위쪽 [테스트 전송]으로 확인할 수 있습니다. 선택 항목입니다.</p>`,
      target: () => biomePage().querySelector('.sec[data-sec="bio-hooks"] .card.form'), input: true, optional: true, needs: 'hookSide' },
    { id: 'done', title: '설정이 완료되었습니다',
      body: `<p>바이옴 매크로 설정이 끝났습니다.</p>
             <p>메인 화면 <b>바이옴 매크로</b>의 <b>시작</b> 버튼이나, 설정 화면 위쪽 스위치를 켜면 알림을 보냅니다.</p>
             <p class="dim">프로그램을 켤 때마다 꺼진 상태로 시작합니다.</p>` },
  ];
  TUTORIALS.push({ id: 'biome', name: '바이옴 매크로 설정', steps: BIOME_STEPS, skipAll: false, menu: 'biome',
    needed: () => needsBiome(),
    skipMsg: '바이옴 매크로 설정 튜토리얼은 다음에 다시 표시됩니다' });

  // ---- 매크로: 메인 화면 ▾ 에서 쓸 기능 켜기 (기능을 켜면 그 기능에 맞춘 튜토리얼이 따로 뜸)
  const MACRO_STEPS = [
    { title: '매크로',
      body: `<p>내 서버에서 돌리는 기능들입니다 (자동 낚시 · 포션 자동 제작 등). 메인 화면 <b>매크로</b> 버튼(F3)으로 시작합니다.</p>
             <p>버튼 옆 <b>▾</b> 에서 쓸 기능만 켜고, 기능을 켜면 그 기능에 필요한 설정만 골라서 따로 안내합니다.</p>`,
      later: true },
    { id: 'more', title: '▾ 를 눌러주세요',
      body: `<p>매크로 버튼 옆 강조된 <b>▾</b> 를 눌러주세요. 기능을 바로 켜고 끄는 목록이 열립니다.</p>`,
      target: () => document.querySelector('[data-quick="macro"]'), done: () => Quick.isOpen('macro') },
    { title: '쓸 기능을 켜주세요',
      body: `<p>쓸 기능의 스위치를 켜주세요. 켜는 순간 그 기능에 필요한 설정을 하나씩 안내합니다.</p>
             <p class="dim">아래 <b>전부 켜기</b> · <b>전부 끄기</b>로 한 번에 바꿀 수도 있습니다. 자세한 설정은 매크로 탭의 <b>매크로 설정</b>에 있습니다.</p>`,
      target: () => document.querySelector('[data-quick-panel="macro"]'), input: true, needs: 'more' },
    { title: '설정이 완료되었습니다',
      body: `<p>메인 화면 <b>매크로</b> 버튼이나 <b>F3</b>으로 켜고 끕니다. 정지는 <b>F7</b> 입니다.</p>` },
  ];
  TUTORIALS.push({ id: 'macro', name: '매크로 기능 켜기', steps: MACRO_STEPS, skipAll: true, menu: 'mfeat',
    needed: () => !doneList().includes('macro') && !macroFeatures(),
    skipMsg: '매크로 튜토리얼을 건너뛰었습니다' });

  // ---- 기능마다 필요한 설정 (켤 때 비어 있는 것만 · 강조는 눌러야 할 줄 하나만) — 전부 매크로 설정 한 화면 안에서
  // 할 일(TASK)마다: ① 자동 보정이 있으면 먼저 해보게 하고 → 안 되거나 직접 하고 싶으면 [다음] → ② 수동으로 한 칸씩
  const MANUAL_TIP = '<p class="dim">안 되거나 직접 지정하고 싶으면 <b>다음</b>을 눌러 하나씩 지정합니다.</p>';
  const FEAT_TIP = {
    mpop: '<p class="dim">레어 바이옴이 뜨면 바이옴별 팝핑의 포션 목록대로 사용합니다.</p>',
    mitem: '<p class="dim">쿨타임마다 아이템을 1개씩 사용합니다.</p>',
    mmerch: '<p class="dim">살 아이템은 상인 자동 구매 칸의 살 아이템에서 고릅니다.</p>',
    mfish: '<p class="dim">자동 낚시 칸 위쪽 상태 확인으로 위치가 맞는지 볼 수 있습니다.</p>',
    mcraft: '<p class="dim">포션 자동 제작 칸의 제작 테스트로 바로 확인할 수 있습니다.</p>',
    mmatch: '<p class="dim">보드 앞에서 오토 메모리 매치 칸의 테스트로 바로 확인할 수 있습니다.</p>',
    mblood: '<p class="dim">블러드 레인 자동 생성 칸의 개수 확인 테스트로 Cursed Rune Fragment 를 찾는지 바로 확인할 수 있습니다 (쓰지 않음).</p>' };
  // 기능 소개 (첫 안내 위쪽)
  const FEAT_INTRO = {
    mcraft: `<p>게임 오른쪽 알림에 <b>Auto Crafted</b> 가 뜨면 Stella 에게 가서 포션 재료를 다시 채웁니다
             (<b>Add Everything → Craft → Add Everything</b>).</p>
             <p><b>제작할 포션</b>을 입력해 두면 그 포션을, 비워 두면 알림에서 읽은 포션을 채웁니다.</p>` };
  // 위치를 다 하고 나서 그 기능 칸에서 할 것 (기능마다 한 번은 꼭 안내)
  const FEAT_SET = {
    mcraft: { items: [['제작할 포션', () => !!(mcraft().potion || '').trim()]],
      steps: () => [
        { title: '제작할 포션을 입력해주세요', target: rowOf('mcraftSet', '#mcraftPotion'), input: true,
          body: `<p>Auto Crafted 알림이 뜨면 여기에 입력한 포션의 재료를 다시 채웁니다. 예: <b>Fortune Potion I</b></p>
                 <p>비워 두면 알림 글자(OCR)에서 포션 이름을 읽습니다. 이름을 잘못 읽을 때는 입력해 두는 것이 정확합니다.</p>
                 <p class="dim">끝의 숫자는 1 · 2 · 3 으로 써도 됩니다 (Fortune Potion 2 → Fortune Potion II). 선택 항목입니다.</p>` },
        { title: '제작 테스트로 확인해보세요', target: rowOf('mcraftSet', '#mcraftTest'), input: true,
          body: `<p><b>테스트</b>를 누르면 위에 입력한 포션으로 지금 한 번 Stella 에게 가서 재료를 채웁니다.</p>
                 <p class="dim">끝날 때까지 손을 떼주세요. 정지는 F7 입니다. 제작할 포션이 비어 있으면 테스트할 수 없습니다. 선택 항목입니다.</p>` },
      ] },
    mmerch: { items: [['살 아이템', () => Object.values(mmerch().buy || {}).some(v => v > 0)]],
      steps: () => [
        { title: '살 아이템을 골라주세요', target: () => $('mmerchItems'), input: true,
          requires: () => Object.values(mmerch().buy || {}).some(v => v > 0),
          body: `<p>마리 · 제스터가 오면 여기서 켠 아이템만 삽니다. 하나 이상 켜주세요.</p>
                 <p class="dim">상인이 와 있을 때 설정의 <b>구매 테스트</b>로 바로 확인할 수 있습니다.</p>` },
      ] },
  };
  const taskBox = feat => ({ base: 'mposBase', mfish: 'mposFish', mmerch: 'mposMerch', mcraft: 'mposCraft', mmatch: 'mposMatch' })[feat];
  // 칸 이름 · 설명 · 지정 방법 (MPOS 표에서)
  function keyInfo(feat, k) {
    const d = MPOS[feat];
    const pt = d.points.find(x => x[0] === k);
    if (pt) return { name: pt[1], sub: pt[2] || '', region: false };
    const rg = [...(d.windows || []), ...d.regions].find(x => x[0] === k);
    return rg ? { name: rg[1], sub: rg[2] || '', region: true } : { name: k, sub: '', region: false };
  }
  // 이 할 일을 안내해야 하는지: 비어 있거나 · 자동 보정을 한 번도 안 했으면 (자동 보정 안내는 기능마다 한 번)
  const taskNeeded = (t, k) => !taskKeysOk(t) || (!taskCalDone(t) && !doneList().includes('cal_' + k));
  const taskRequired = t => !t.optional && !taskKeysOk(t);
  function taskSteps(id, t) {
    const sideId = 'side_' + id, out = [];
    out.push({ id: sideId, ...secStep(mfeatPage, t.sec, t.side), done: () => mfSec(t.sec), needs: 'card' });
    const opt = !!t.optional;
    if (t.auto) {
      const done = () => (t.cal ? taskCalDone(t) : false) || ((t.keys || []).length > 0 && taskKeysOk(t));
      out.push({ id: 'auto_' + id, title: t.auto.title, body: t.auto.body + ((t.keys || []).length ? MANUAL_TIP : ''),
        target: t.auto.target, input: true, done, needs: sideId, optional: opt, skipTo: 'end_' + id });
    }
    if (t.places) {
      const all = move().places || [];
      const mine = all.map((pl, i) => [pl, i]).filter(([pl]) => (pl.feat || 'mfish') === t.places);
      if (!mine.length) {
        out.push({ title: `${t.label}${eul(t.label)} 지정해주세요`, target: () => $(taskBox(t.places))?.querySelector(`[data-mpos-custom="places:${t.places}"]`),
          body: `<p>지점을 <b>기준 장소에서 지정</b>한 뒤 <b>시간 재기</b>로 걸리는 시간을 재주세요.</p>`,
          input: true, requires: () => placesOk(t.places), done: () => placesOk(t.places), needs: sideId });
      }
      for (const [pl, i] of mine) pl.points.forEach((pt0, j) => {
        const name = pl.name || t.label, row = rowOf(taskBox(t.places), `[data-move-pick="${i},${j},1"]`);
        const pt = () => (move().places || [])[i]?.points?.[j] || {};
        out.push({ title: `${name} · ${j + 1}번 지점을 지정해주세요`, target: row, input: true,
          body: `<p><b>기준 장소에서 지정</b>을 누르면 캐릭터가 기준 장소로 갑니다. 도착하면 로블록스 화면에서 걸어갈 곳을 클릭해주세요.</p>`,
          requires: () => !!pt().pos, done: () => !!pt().pos, needs: sideId });
        out.push({ title: `${name} · ${j + 1}번 지점 시간을 재주세요`, target: row, input: true,
          body: `<p><b>시간 재기</b>를 누르면 기준 장소에서 그 지점까지 걸어갑니다. 캐릭터가 도착하면 <b>F6</b> 을 눌러주세요.</p>`,
          requires: () => pt().time != null, done: () => pt().time != null, needs: sideId });
      });
    }
    for (const q of t.steps || [])
      out.push({ title: q.title, body: q.body, target: q.target, input: true, requires: q.ok, done: q.ok, needs: sideId });
    for (const k of t.manual || t.keys || []) {
      const { name, sub, region } = keyInfo(t.feat, k);
      const has = () => !!featCfg(t.feat)[k];
      // 수동 칸은 자동 방법이 끝났으면 건너뜀 (자동 보정이 안쪽 위치를 다 채움)
      const done = () => has() || (!!t.auto && taskKeysOk(t));
      out.push({ title: region ? `${name}${eul(name)} 지정해주세요` : `${name} 위치를 지정해주세요`, input: true,
        target: rowOf(taskBox(t.feat), `[data-mpos-${region ? 'region' : 'pick'}="${k}"]`),
        body: (region ? `<p><b>드래그로 지정</b>을 누른 뒤 로블록스 화면에서 드래그해주세요.</p>`
                      : `<p><b>위치 지정</b>을 누른 뒤 로블록스 화면에서 클릭해주세요.</p>`) + (sub ? `<p class="dim">${sub}</p>` : ''),
        requires: opt ? null : done, done, optional: opt, skipTo: opt ? 'end_' + id : undefined, needs: sideId });
    }
    return out;
  }
  function featSteps(k) {
    const ids = FEAT_TASKS[k] || [];
    const todo = ids.filter(id => taskNeeded(TASKS[id], k));
    const fs = FEAT_SET[k];
    const reqList = ids.map(id => `<li class="${taskKeysOk(TASKS[id]) && taskCalDone(TASKS[id]) ? 'ok' : ''}">${TASKS[id].label}${TASKS[id].optional ? ' (선택)' : ''}</li>`)
      .concat((fs?.items || []).map(([label, ok]) => `<li class="${ok() ? 'ok' : ''}">${label}</li>`));
    if (k === 'mpop' || k === 'mblood') reqList.unshift(`<li class="${(config.biome?.player || '').trim() ? 'ok' : ''}">플레이어 이름 (바이옴 매크로 설정)</li>`);
    const steps = [
      { title: `${FEAT_NAME[k]} 설정`,
        body: (FEAT_INTRO[k] || '') + `<p>이 기능에 필요한 것만 차례대로 안내합니다. 전부 <b>매크로 설정</b> 한 화면 안에 있습니다.</p>
               <ul class="tut-req">${reqList.join('')}</ul>
               <p class="dim">자동 보정이 있는 것은 자동 보정부터 해보고, 안 되면 수동으로 진행합니다.</p>`,
        later: true, task: '' },
      { id: 'card', ...cardStep('mfeat', '매크로 설정'), task: '',
        body: `<p>매크로 탭의 강조된 <b>매크로 설정</b> 버튼을 눌러주세요.</p>` }];
    const groups = todo.map(id => taskSteps(id, TASKS[id]));
    const total = groups.length + (fs ? 1 : 0);
    groups.forEach((g, n) => {
      for (const s of g) s.task = `${n + 1} / ${total} · ${TASKS[todo[n]].label}`;   // 단계 수 대신 할 일 순서
      // 건너뛰기 → 다음 할 일의 첫 단계로
      const nextId = n + 1 < groups.length ? groups[n + 1][0].id : fs ? 'fx_side' : 'final';
      for (const s of g) if (s.skipTo === 'end_' + todo[n]) s.skipTo = nextId;
      steps.push(...g);
    });
    if (fs) {
      const task = `${total} / ${total} · 기능 설정`;
      steps.push({ id: 'fx_side', task, ...secStep(mfeatPage, FEAT_SEC[k], FEAT_NAME[k]), done: () => mfSec(FEAT_SEC[k]), needs: 'card' });
      for (const q of fs.steps()) steps.push({ ...q, task, needs: 'fx_side' });
    }
    steps.push({ id: 'final', task: '', title: '설정이 완료되었습니다',
      body: `<p>메인 화면 <b>매크로</b> 버튼이나 <b>F3</b>으로 켜면 작동합니다. 정지는 <b>F7</b> 입니다.</p>${fs ? '' : FEAT_TIP[k] || ''}` });
    return steps;
  }
  for (const k of Object.keys(FEAT_TASKS))
    TUTORIALS.push({ id: 'feat_' + k, name: `${FEAT_NAME[k]} 설정`, build: () => featSteps(k), steps: [], skipAll: true, menu: 'mfeat',
      needed: () => false, skipMsg: '설정 안내를 건너뛰었습니다 · 매크로 설정의 그 기능 칸 [설정 안내]에서 다시 볼 수 있습니다' });

  // 단계 참조(needs · skipTo)를 이름으로 쓸 수 있게 → 번호로 바꿈 (단계를 추가해도 번호가 안 꼬임)
  function resolveRefs(t) {
    const idx = {};
    t.steps.forEach((s, n) => { if (s.id) idx[s.id] = n; });
    for (const s of t.steps) for (const k of ['needs', 'skipTo']) {
      if (typeof s[k] === 'string') {
        if (!(s[k] in idx)) console.error('튜토리얼 단계 참조 오류', t.id, s[k]);
        s[k] = idx[s[k]];
      }
    }
  }
  TUTORIALS.forEach(t => { if (!t.build) resolveRefs(t); });
  // 모드 튜토리얼: Tutorial.register(kit => [튜토리얼, …]) — 바이옴 매크로 다음 · 매크로 앞에 들어감
  const KIT = { cardStep, secStep, backStep, pickRow, pageOf, onSec, sectionIs, doneList, eul, cardOf };
  function register(fn) {
    const list = fn(KIT) || [];
    const at = Math.max(1, TUTORIALS.findIndex(t => t.id === 'macro'));
    for (const t of list) if (!t.build) resolveRefs(t);
    TUTORIALS.splice(at, 0, ...list);
  }

  let tut = TUTORIALS[0];
  let STEPS = tut.steps;
  const skipped = new Set();     // 이번 실행에서 스킵한 튜토리얼 (남은 개수에서 제외)
  // 남은 튜토리얼: 지금 하는 것 + 아직 필요한 것만 (설정이 이미 돼 있어서 안 뜰 튜토리얼은 세지 않음)
  const remaining = () => TUTORIALS.filter(t => !doneList().includes(t.id) && !skipped.has(t.id)
    && (t === tut || t.needed())).length;
  function markDone(id) {
    const add = [id];
    if (id.startsWith('feat_')) add.push('cal_' + id.slice(5), 'fx_' + id.slice(5));   // 기능 안내를 봤으면 자동 보정 · 기능 설정 안내도 본 것
    const list = [...doneList(), ...add.filter(x => !doneList().includes(x))];
    if (list.length !== doneList().length) {
      config.tutorials_done = list;
      queueSave({ tutorials_done: list });
    }
  }

  // 스포트라이트/안내창 위치. animate=false 면 이동 애니메이션 없이 바로 배치
  function place(animate = true) {
    const cr = card.getBoundingClientRect(), cw = cr.width, ch = cr.height, m = 16 * Z;
    let left, top;
    if (focusEl) {
      // 강조할 곳이 스크롤 밖에 있으면 먼저 보이게
      const sc = focusEl.closest('.sec');
      if (sc) {
        const fr = focusEl.getBoundingClientRect(), sr = sc.getBoundingClientRect();
        if (fr.top < sr.top || fr.bottom > sr.bottom) focusEl.scrollIntoView({ block: 'nearest' });
      }
      const r = focusEl.getBoundingClientRect(), pad = 8 * Z;
      if (spot.style.display !== 'block') {
        // 강조 박스는 안내창 자리에서 출발해서 대상으로 날아감
        noAnim(spot, () => Object.assign(spot.style, { display: 'block', ...box(cr) }));
      }
      Object.assign(spot.style, { left: u(r.left - pad) + 'px', top: u(r.top - pad) + 'px',
        width: u(r.width + pad * 2) + 'px', height: u(r.height + pad * 2) + 'px' });
      el.classList.add('spot');
      [left, top] = cardSpot(r, cw, ch, m, pad);
    } else {
      spot.style.display = 'none';
      el.classList.remove('spot');
      left = (innerWidth - cw) / 2; top = (innerHeight - ch) / 2;
    }
    const pos = { left: u(left) + 'px', top: u(top) + 'px', transform: 'none' };
    if (animate) Object.assign(card.style, pos);
    else noAnim(card, () => Object.assign(card.style, pos));
  }
  // 안내창 자리: 아래 → 위 → 오른쪽 → 왼쪽 → 화면 구석 순서로, 강조된 곳을 안 가리는 첫 자리
  // 어디든 가리게 되면(강조된 곳이 아주 클 때) 가장 덜 가리는 자리
  function cardSpot(r, cw, ch, m, pad) {
    const g = 20 * Z, W = innerWidth, H = innerHeight;
    const cx = x => Math.min(Math.max(m, x), W - cw - m);
    const cy = y => Math.min(Math.max(m, y), H - ch - m);
    const T = { l: r.left - pad, t: r.top - pad, r: r.right + pad, b: r.bottom + pad };
    const cands = [
      [r.left, r.bottom + g], [r.left, r.top - ch - g],
      [r.right + g, r.top], [r.left - cw - g, r.top],
      [W - cw - m, H - ch - m], [m, H - ch - m], [W - cw - m, m], [m, m],
    ].map(([x, y]) => [cx(x), cy(y)]);
    let best = cands[0], bestArea = Infinity;
    for (const [x, y] of cands) {
      const ow = Math.min(x + cw, T.r) - Math.max(x, T.l), oh = Math.min(y + ch, T.b) - Math.max(y, T.t);
      const area = ow > 0 && oh > 0 ? ow * oh : 0;
      if (area === 0) return [x, y];
      if (area < bestArea) { bestArea = area; best = [x, y]; }
    }
    return best;
  }
  function noAnim(node, fn) {
    node.classList.add('no-move');
    fn();
    void node.offsetWidth;
    node.classList.remove('no-move');
  }

  function show(n) {
    // 앞 단계 조건이 깨졌으면 (예: 화면을 닫음) 그 단계로 되돌아감
    while (STEPS[n]?.needs !== undefined && !STEPS[STEPS[n].needs].done()) n = STEPS[n].needs;
    // 이미 해둔 조작 단계는 건너뜀
    while (STEPS[n]?.done && STEPS[n].done() && n < STEPS.length - 1) n++;
    i = n;
    const s = STEPS[i];
    el.classList.remove('passive');
    card.classList.remove('in');
    const last = i === STEPS.length - 1;
    $('tutStep').textContent = s.task ?? `${i + 1} / ${STEPS.length}`;
    const left = remaining() - (last && !doneList().includes(tut.id) && !skipped.has(tut.id) ? 1 : 0);   // 마지막 단계에선 이번 것 제외
    $('tutLeft').textContent = left > 0 ? `남은 튜토리얼 ${left}개` : '모든 튜토리얼 완료';
    $('tutTop').style.display = i === 0 ? '' : 'none';   // 각 튜토리얼 첫 단계에서만 표시 (없으면 그만큼 줄어듦)
    $('tutName').textContent = tut.name;
    $('tutTitle').textContent = s.title;
    // 단계별 예시 GIF (web/tut/이름.gif)
    $('tutBody').innerHTML = (s.gif ? `<div class="tut-gif"><img src="tut/${s.gif}.gif" alt=""></div>` : '') + s.body;
    $('tutBody').querySelector('.tut-gif img')?.addEventListener('load', () => place(false), { once: true });
    $('tutPrev').style.visibility = i > 0 && !last ? '' : 'hidden';
    // 전체 스킵: 감시 대상 튜토리얼은 항상 / 바이옴 튜토리얼은 첫 안내에서만 [나중에]
    const skipAll = !last && (tut.skipAll || s.later);
    $('tutSkip').style.display = skipAll ? '' : 'none';
    skipCount = 0;
    $('tutSkip').textContent = s.later && !tut.skipTimes ? '나중에' : '스킵';
    $('tutStepSkip').style.display = s.optional ? '' : 'none';   // 선택 단계만 건너뛰기 가능
    const next = $('tutNext');
    next.style.display = s.target && !s.input ? 'none' : '';      // 조작 단계는 직접 눌러야 넘어감
    next.textContent = last ? '확인' : '다음';
    updateReq();
    I18N.apply(card);                  // 번역된 글자 크기로 위치를 잡도록 먼저 번역
    const first = !moved;
    moved = true;
    focusEl = s.target ? s.target() : null;
    if (!focusEl?.closest('.main-tile')) Quick.hide();         // 메인 화면 ▾ 목록이 강조할 곳을 가리지 않게
    const tabEl = focusEl?.closest('.menu-tab');
    // 탭이 미끄러져 들어오는 동안 강조 박스는 아래 follow 가 따라가며 제자리를 잡음
    if (tabEl && !tabEl.classList.contains('on')) setTab(tabEl.dataset.tab);
    place(!first);
    startFollow();
    requestAnimationFrame(() => card.classList.add('in'));
  }

  // 필수 입력 단계: 조건을 채워야 [다음] 가능
  function updateReq() {
    const s = STEPS[i];
    const ok = !s?.requires || s.requires();
    $('tutNext').disabled = !ok;
    $('tutNext').title = ok ? '' : '입력해야 다음으로 넘어갈 수 있습니다';
  }

  // 조작이 일어날 때마다 현재 단계가 끝났는지 확인
  function check() {
    if (el.hidden || i < 0 || busy) return;
    const s = STEPS[i];
    if (s.done && s.done()) show(i + 1);
    else if (s.needs !== undefined && !STEPS[s.needs].done()) show(s.needs);
    else place();
  }

  function end() {
    el.hidden = true;
    i = -1;
    focusEl = null;
    moved = false;
    spot.style.display = 'none';
  }

  $('tutNext').addEventListener('click', () => {
    if (STEPS[i]?.requires && !STEPS[i].requires()) return;
    if (i < STEPS.length - 1) return show(i + 1);
    const id = tut.id;
    markDone(id);       // 끝까지 완료한 튜토리얼만 완료로 기록
    end();
    afterTutorial(id);
  });
  $('tutStepSkip').addEventListener('click', () => {
    const s = STEPS[i];
    if (!s?.optional) return;
    show(Math.min(STEPS.length - 1, s.skipTo ?? i + 1));
  });
  document.addEventListener('input', () => { if (!el.hidden) updateReq(); });
  $('tutPrev').addEventListener('click', () => {
    let n = i - 1;
    while (n > 0 && STEPS[n].done && STEPS[n].done()) n--;   // 이미 끝낸 조작 단계는 건너뛰고 돌아감
    if (n >= 0) { i = n; show(n); }
  });
  $('tutSkip').addEventListener('click', () => {
    if (!(tut.skipAll || STEPS[i]?.later)) return;
    // 스킵을 여러 번 눌러야 하는 튜토리얼 (실수로 넘기지 않게)
    if (tut.skipTimes && ++skipCount < tut.skipTimes) {
      $('tutSkip').textContent = `스킵 (${tut.skipTimes - skipCount}번 더)`;
      return;
    }
    const msg = tut.skipMsg, id = tut.id;
    skipped.add(id);
    if (id.startsWith('feat_')) { markDone('cal_' + id.slice(5)); markDone('fx_' + id.slice(5)); }   // 건너뛰면 자동 보정 · 기능 설정 안내는 다시 안 띄움
    end();
    toast(msg);
    afterTutorial(id);   // 스킵은 이 튜토리얼만 — 남은 튜토리얼은 이어서 진행
  });
  // 메인 화면 버튼 순서(위 → 아래)대로 이어서 진행: 바이옴 매크로 설정 → 디스코드 감지 설정
  function afterTutorial(id) {
    const k = TUTORIALS.findIndex(t => t.id === id);
    const next = TUTORIALS.slice(k + 1).find(t => t.needed());
    if (chain && next) setTimeout(() => start(next.id, true), 400);
  }
  addEventListener('resize', () => { if (!el.hidden) requestAnimationFrame(() => place(false)); });
  // 강조할 곳이 움직이면(탭이 미끄러져 들어옴 · 글자 길이 변화 · 화면 배치 변경 등) 강조 박스 · 안내창이 따라감
  let lastRect = '', following = false;
  function follow() {
    if (el.hidden) { following = false; lastRect = ''; return; }     // 튜토리얼이 닫히면 멈춤 (매 프레임 돌지 않게)
    if (focusEl && !busy) {
      const r = focusEl.getBoundingClientRect();
      const key = [r.left, r.top, r.width, r.height].map(v => Math.round(v)).join(',');
      if (key !== lastRect) {
        const first = !lastRect;
        lastRect = key;
        if (!first) place();
      }
    } else lastRect = '';
    requestAnimationFrame(follow);
  }
  const startFollow = () => { if (!following) { following = true; requestAnimationFrame(follow); } };

  // 튜토리얼 중에는 안내창과 강조된 곳만 누를 수 있음 (나머지 버튼·단축키 차단)
  const allowed = t => card.contains(t) || !!t.closest?.('.lang') || (!el.classList.contains('passive') && focusEl && focusEl.contains(t));
  for (const type of ['pointerdown', 'mousedown', 'mouseup', 'click', 'dblclick', 'contextmenu', 'auxclick']) {
    addEventListener(type, e => {
      if (el.hidden || allowed(e.target)) return;
      e.preventDefault();
      e.stopImmediatePropagation();
    }, true);
  }
  addEventListener('keydown', e => {
    if (el.hidden) return;
    const t = document.activeElement;
    if (t && t !== document.body && allowed(t) && e.key !== 'Escape') return;   // 강조된 입력칸에 입력은 허용
    if (e.key === 'Tab' || e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  }, true);
  $('bioTutorialBtn').addEventListener('click', () => start('biome'));
  // 게임 입장 위치 안내는 스나이핑 모드의 오토 팝핑 튜토리얼에 있음 (모드가 없으면 버튼을 숨김 · init)
  $('playTutorialBtn').addEventListener('click', () => start('popping', false, { play: true }));
  document.querySelectorAll('[data-feat-tut]').forEach(b => b.addEventListener('click', () => {
    end(); Tutorial.featureOn(b.dataset.featTut, true);
  }));

  // 튜토리얼은 항상 메인 화면에서 시작 (다른 화면이 열려 있으면 먼저 닫음)
  // chain: 자동으로 뜬 경우만 끝난 뒤 남은 튜토리얼로 이어감 ([튜토리얼 보기] 등 직접 연 경우는 그것만)
  // opts: 단계를 새로 만드는 튜토리얼(build)에 넘김 (예: 게임 입장 위치가 있어도 보기)
  async function start(id = 'biome', autoChain = false, opts = {}) {
    end();
    chain = autoChain;
    skipped.delete(id);
    tut = TUTORIALS.find(t => t.id === id) || TUTORIALS[0];
    if (tut.build) { tut.steps = tut.build(opts); resolveRefs(tut); }
    STEPS = tut.steps;
    // 색 테마: 그 기능 버튼의 색으로
    el.style.setProperty('--tc', MENU.find(m => m.key === tut.menu)?.color || 'var(--accent)');
    while (busy) await wait(50);
    if (current) {
      await closePage();
      while (busy) await wait(50);
    }
    el.hidden = false;
    i = 0;
    show(0);
  }

  return {
    start,
    register,
    has: id => TUTORIALS.some(t => t.id === id),
    maybeStart() {
      const first = TUTORIALS.find(t => t.needed());
      if (first) start(first.id, true);
    },
    isOpen: () => !el.hidden,
    peek: () => ({ id: tut.id, i, n: STEPS.length, el: focusEl }),     // 지금 단계 (확인용)
    // 기능을 켰을 때: 그 기능에 비어 있는 설정이 있으면 맞춤 튜토리얼 (force: 다 돼 있어도 보기)
    featureOn(k, force) {
      if (!FEAT_TASKS[k]) return;
      if (!el.hidden && tut.id === 'macro') { markDone('macro'); end(); }
      else if (!el.hidden) return;
      const tasks = FEAT_TASKS[k].map(id => TASKS[id]);
      // 필수가 비어 있거나 · 자동 보정을 아직 한 번도 안내 안 했으면
      if (force || tasks.some(taskRequired) || tasks.some(t => taskNeeded(t, k))
          || (FEAT_SET[k] && !doneList().includes('fx_' + k))) start('feat_' + k);
    },
    // 화면이 다시 그려졌거나(위치 지정 후) 필수 값이 바뀌었을 때: 강조 대상·[다음] 상태 갱신 · 끝난 단계면 다음으로
    refresh() {
      if (el.hidden || i < 0 || busy) return;
      const s = STEPS[i];
      if (s.done && s.done() && i < STEPS.length - 1) return show(i + 1);
      if (s.target) focusEl = s.target();
      updateReq();
      place();
    },
    // 화면 전환 애니메이션 동안은 숨김
    onNavigate() { if (!el.hidden) el.classList.add('passive'); },
    onNavigated() { if (!el.hidden) check(); },
    onTargetAdded() { if (!el.hidden) check(); },
    onSection() { if (!el.hidden) check(); },
  };
})();

window.addEventListener('acrux-desktop-error', e => toast(e.detail));

// ---------------------------------------------------------------- 모드 · 플러그인
// 모드 = 설정 탭 · 화면이 있는 큰 기능 (예: 스나이핑) · 플러그인 = 화면 없이 뒤에서 도는 작은 기능
// 모드 화면: mod.json 의 ui.html (화면 · data-mod-into="선택자" 가 붙은 건 그 안으로) · ui.css · ui.js — 켤 때 init 에서 붙임
// 모드 JS 는 이 파일의 함수 · 값을 그대로 씀 (api · config · queueSave · toast · $ · MENU · TABS …) + 아래 Mods
const Mods = (() => {
  const hooks = {};
  let list = [];
  const on = (ev, fn) => (hooks[ev] ||= []).push(fn);
  // 이벤트 → 등록한 함수의 결과 목록 (한 모드가 오류를 내도 나머지는 계속)
  function emit(ev, ...a) {
    const out = [];
    for (const fn of hooks[ev] || []) {
      try { out.push(fn(...a)); } catch (e) { console.warn('mod', ev, e); }
    }
    return out;
  }
  // 값을 차례로 고쳐서 넘김 (예: 복귀 상태에 '커스텀 매크로 실행 중' 덧붙이기)
  function pipe(ev, v) {
    for (const fn of hooks[ev] || []) { try { v = fn(v) ?? v; } catch (e) { console.warn('mod', ev, e); } }
    return v;
  }
  const addMenu = (...entries) => MENU.splice(MENU.findIndex(m => m.tab === 'macro'), 0, ...entries);
  // 탭 이름 (before: 이 탭 앞에 · 기본 매크로 앞)
  function addTab(key, label, before = 'macro') {
    if (TABS.includes(key)) return;
    const i = TABS.indexOf(before);
    TABS.splice(i < 0 ? TABS.length : i, 0, key);
    const b = document.createElement('button');
    b.className = 'tab-name'; b.type = 'button'; b.dataset.tab = key; b.textContent = label;
    const ref = document.querySelector(`.tab-name[data-tab="${before}"]`);
    $('tabTrack').insertBefore(b, ref);
  }
  const loadScript = src => new Promise(res => {
    const sc = document.createElement('script');
    sc.src = src; sc.onload = () => res(true); sc.onerror = () => res(false);
    document.body.appendChild(sc);
  });
  async function load(mods) {
    list = mods || [];
    for (const m of list) {
      if (!m.loaded || !m.ui) continue;
      try {
        if (m.ui.css) {
          const l = document.createElement('link');
          l.rel = 'stylesheet'; l.href = m.ui.css;
          document.head.appendChild(l);
        }
        if (m.ui.html) {
          const t = document.createElement('template');
          t.innerHTML = await (await fetch(m.ui.html, { cache: 'no-store' })).text();
          for (const el of [...t.content.children]) {
            const into = el.dataset.modInto && document.querySelector(el.dataset.modInto);
            if (into) { el.removeAttribute('data-mod-into'); into.appendChild(el); }
            else document.body.insertBefore(el, $('tut'));
          }
        }
        if (m.ui.js && !(await loadScript(m.ui.js))) console.warn('mod js', m.id);
      } catch (e) { console.warn('mod ui', m.id, e); }
    }    // 메인 타일을 탭 순서대로 (예: 바이옴 → 스나이핑 → 매크로) — 원래 있던 것처럼
    const box = $('mains'), at = el => {
      const i = TABS.indexOf(el.querySelector('[data-tab-go]')?.dataset.tabGo);
      return i < 0 ? TABS.length : i;
    };
    [...box.children].sort((a, b) => at(a) - at(b)).forEach(el => box.appendChild(el));
  }
  return { on, emit, pipe, addMenu, addTab, load, list: () => list, has: id => list.some(m => m.id === id && m.loaded) };
})();

// 추가 기능 탭 → 모드 · 플러그인 화면
let modsChanged = false;
function renderModList(mods) {
  const box = $('modList');
  if (!mods.length) {
    box.innerHTML = '<div class="empty">설치된 모드 없음 · 위쪽 [모드 파일 설치] 로 .acxm 파일을 고르기</div>';
  } else {
    box.innerHTML = mods.map(m => `
      <div class="card mod-card" data-mod="${esc(m.id)}">
        <div class="mod-head"><b>${esc(m.name)}</b><span class="mod-tag">${m.type === 'mod' ? '모드' : '플러그인'}</span>
          <span class="mod-ver" data-notr>V${esc(m.version || '?')}</span><span class="grow"></span>
          <label class="switch" title="켜기 · 끄기"><input type="checkbox" data-mod-on="${esc(m.id)}" ${m.enabled ? 'checked' : ''}><i></i></label></div>
        <p class="mod-desc">${esc(m.description || '')}</p>
        <div class="mod-foot"><small data-notr>${esc(m.author ? 'by ' + m.author : '')}</small><span class="grow"></span>
          <small class="mod-st${m.error ? ' err' : ''}">${m.error ? '오류: ' + esc(m.error) : m.loaded ? '작동 중' : m.enabled ? '다시 켜면 작동' : '꺼짐'}</small>
          <button class="btn ghost mini" type="button" data-mod-rm="${esc(m.id)}">삭제</button></div>
      </div>`).join('');
  }
  $('modRestart').hidden = !modsChanged;
}
async function refreshMods() {
  try { const r = await api('mods_list'); renderModList(r.mods || []); } catch { /* 다음에 */ }
}
document.addEventListener('change', async e => {
  const t = e.target.closest('[data-mod-on]');
  if (!t) return;
  const r = await api('mods_toggle', { id: t.dataset.modOn, on: t.checked });
  if (r.error) return toast(r.error);
  modsChanged = true; renderModList(r.mods || []);
  toast(`${t.checked ? '켬' : '끔'} · 다시 켜면 적용`);
});
document.addEventListener('click', async e => {
  const t = e.target.closest('[data-mod-rm]');
  if (!t) return;
  if (t.dataset.sure !== '1') { t.dataset.sure = '1'; t.textContent = '정말 삭제'; setTimeout(() => { t.dataset.sure = ''; t.textContent = '삭제'; }, 3000); return; }
  const r = await api('mods_remove', { id: t.dataset.modRm });
  if (r.error) return toast(r.error);
  modsChanged = true; renderModList(r.mods || []); toast('삭제함 · 다시 켜면 적용');
});
$('modInstall').addEventListener('click', async e => {
  const b = e.currentTarget;
  b.disabled = true;
  try {
    const r = await api('mods_install', {}, 320000);
    if (r.cancel) return;
    if (r.error) return toast(r.error);
    modsChanged = true; renderModList(r.mods || []);
    toast(`${r.name} 설치함 · 다시 켜면 적용`);
  } finally { b.disabled = false; }
});
$('modFolder').addEventListener('click', () => api('mods_folder'));
$('modRestart').addEventListener('click', async () => {
  $('modRestart').disabled = true;
  const r = await api('restart');
  toast(r.error || 'Acrux 를 다시 켜는 중…');
  if (r.error) $('modRestart').disabled = false;
});

// ---------------------------------------------------------------- 시작
setTimeout(() => document.body.classList.remove('booting'), 4000);   // 켜다가 막혀도 화면은 보이게
(async function init() {
  I18N.init();
  const s = await api('state');
  config = s.config;
  await Mods.load(s.mods);               // 모드 화면 · 탭 · 메뉴를 먼저 붙이고 메뉴를 만듦
  buildMenu();
  buildLang($('langBox'), false);
  buildLang($('tutLangBox'), true);
  if (s.version) $('verText').textContent = 'V' + s.version;
  I18N.set(config.lang || I18N.detect());
  if (!config.lang) queueSave({ lang: I18N.lang });   // 처음 실행: 윈도우 언어로 정해서 저장
  I18N.onChange(() => { Tutorial.refresh(); requestAnimationFrame(() => setTab(curTab, false)); });   // 글자 길이가 바뀌면 간격 다시
  setTab(config.menu_tab || 'macro', false);
  document.body.classList.remove('booting');
  fillPlay();
  fillPop();
  fillRet();
  fillBiome();
  fillMpop();
  renderMfish();
  renderMitem(); renderMmerch(); renderMcraft(); renderMmatch(); renderMblood();
  renderMpos('base'); renderMpos('mfish'); renderMpos('mmerch'); renderMpos('mcraft'); renderMpos('mmatch');
  fillAcrux();
  renderModList(s.mods || []);
  $('playTutorialBtn').hidden = !Tutorial.has('popping');
  Mods.emit('init', s);                  // 모드: 화면 채우기 · 상태
  syncAllReady();
  renderSummary();
  setStatus(s.status);
  renderSummary();
  poll();
  setTimeout(() => Tutorial.maybeStart(), 500);   // 비어 있는 필수 설정이 있으면 튜토리얼
})();
