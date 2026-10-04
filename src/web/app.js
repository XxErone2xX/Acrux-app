'use strict';

// ---------------------------------------------------------------- 서버 연결
const TOKEN = location.hash.slice(1);
history.replaceState(null, '', location.pathname);

async function api(name, payload = {}) {
  const r = await fetch('/api/' + name, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-Token': TOKEN },
    body: JSON.stringify(payload),
  });
  if (!r.ok) throw new Error(name + ' ' + r.status);
  return r.json();
}

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
const MENU = [
  { key: 'biome', tab: 'biome', icon: 'globe', color: 'var(--cyan)', title: '바이옴 매크로 설정', desc: '바이옴 알림 · 디스코드 웹후크' },
  { key: 'discord', tab: 'snipe', icon: 'chat', color: 'var(--accent)', title: '디스코드 감지 설정', desc: '감시할 서버·채널, 필터, 감지 기록' },
  { key: 'popping', tab: 'snipe', icon: 'bolt', color: 'var(--orange)', title: '오토 팝핑 매크로 설정', desc: '게임 접속 후 Play 버튼 자동 클릭' },
  { key: 'return', tab: 'snipe', icon: 'undo', color: 'var(--green)', title: '매크로 복귀 설정', desc: '바이옴이 끝나면 내 서버로 복귀' },
  { key: 'mfeat', tab: 'macro', icon: 'bolt', color: 'var(--yellow)', title: '매크로 기능 설정', desc: '내 서버에서 돌릴 기능 켜기 · 끄기' },
  { key: 'mpos', tab: 'macro', icon: 'pin', color: 'var(--red)', title: '매크로 기준 위치 설정', desc: '기능마다 버튼 위치 · 영역 지정' },
  { key: 'mstats', tab: 'macro', icon: 'chart', color: 'var(--cyan)', title: '통계 보기', desc: '이번 실행 · 올타임 기록' },
  { key: 'acrux', tab: 'acrux', icon: 'gear', color: '#7c6cf6', title: 'Acrux 설정', desc: 'OCR 감지 방식 · 언어 · 데이터 폴더',
    grad: 'linear-gradient(135deg, #8fa0ff, #6c7bff 50%, #8b5cf6)' },
];
// 설정 탭 — 끝없이 돌아감 (바이옴 → 스나이프 → 매크로 → Acrux → 바이옴 …)
const TABS = ['biome', 'snipe', 'macro', 'acrux'];
const TAB_SCALE = 1.65, TAB_GAP = 40;     // 지금 탭 확대 배율 · 옆 탭과 보이는 간격(px)
let curTab = 'snipe';
const wrapIdx = n => (n % TABS.length + TABS.length) % TABS.length;
function setTab(tab, animate = true, dir = 0) {
  if (!TABS.includes(tab)) tab = 'snipe';
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
    b.classList.toggle('far', Math.abs(r) > 1);       // 탭이 4개: 반대편 탭 이름은 숨김
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
  <div><b>${m.title}${m.soon ? '<span class="tag">준비 중</span>' : ''}</b><span class="desc">${m.desc}</span></div>`;

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
// 스나이핑 안정성 설정: 검정 + 빨강 (스나이프 탭 맨 아래)
const SNIPE_ENTRY = { key: 'snipe', icon: 'shield', color: '#e5484d', title: '스나이핑 안정성 설정', desc: '사람처럼 접속 · 안티 스나이핑 대비',
  grad: 'linear-gradient(135deg, #1a0607, #8e1c20 55%, #e5484d)', decos: ['#e5484d', '#8e1c20', '#8e1c20', '#e5484d'] };
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
  // Donate: 크레딧 탭 밖 (메인 왼쪽 아래) — 링크는 준비 중
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
  // 스나이핑 안정성 설정: 오른쪽 위에서 스나이프 탭 맨 아래로 옮김
  for (const m of [...MENU, { ...SNIPE_ENTRY, tab: 'snipe', cls: 'snipe-open' }]) {
    const b = document.createElement('button');
    b.className = 'menu-card deco' + (m.cls ? ' ' + m.cls : '');
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
  if (key === 'mfeat') renderMpop();
  if (key === 'acrux') refreshOcrInfo();          // 플레이어 이름 · 버튼 위치가 바뀌었을 수 있음
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

function selectSection(page, sec) {
  if (!sec) return;
  page.querySelectorAll('.side .item').forEach(i => i.classList.toggle('active', i.dataset.sec === sec));
  page.querySelectorAll('.content .sec').forEach(s => s.classList.toggle('active', s.dataset.sec === sec));
  if (sec === 'log') scrollLog(true);
  if (typeof Tutorial !== 'undefined') Tutorial.onSection();
}
document.querySelectorAll('.page').forEach(page => {
  page.querySelectorAll('.side .item').forEach(it => it.addEventListener('click', () => selectSection(page, it.dataset.sec)));
});

// ---------------------------------------------------------------- 상태 / 설정
let config = {};
let seq = 0;
let armed = false;     // 시작 버튼 상태 (꺼져 있으면 감지만 하고 작동 안 함)
const $ = id => document.getElementById(id);

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

function targetText(c) {
  const g = (c.guild_ids || []).length, ch = (c.channel_ids || []).length;
  if (!g && !ch) return '감시 대상 없음';
  return [g ? `서버 ${g}개 전체` : '', ch ? `채널 ${ch}개` : ''].filter(Boolean).join(' + ');
}

function renderSummary() {
  $('targetSum').textContent = `감시 대상: ${targetText(config)}`;
  syncMainTiles();
}

const FIELDS = [
  ['cfgEnabled', 'enabled', 'check'],
  ['cfgFlavor', 'discord', 'text'],
  ['cfgAutoRestart', 'auto_restart_discord', 'check'], ['cfgAutostart', 'autostart', 'check'],
  ['cfgBeep', 'beep', 'check'], ['cfgShowAll', 'show_all_links', 'check'], ['cfgStable', 'stable_join', 'check'],
];

function fillFields() {
  for (const [id, key, type] of FIELDS) {
    const el = $(id), v = config[key];
    if (type === 'check') el.checked = !!v;
    else if (type === 'lines') el.value = (v || []).join('\n');
    else el.value = v ?? '';
  }
}

let pending = {}, saveTimer = null;
const LOCAL_KEYS = ['play', 'biome', 'pop', 'ret', 'snipe', 'mpop', 'mfish'];
function queueSave(patch) {
  Object.assign(pending, patch);
  // 팝핑·바이옴·매크로 탭 설정은 화면 쪽 객체가 원본 (폼이 그 객체를 직접 고치므로 복사본으로 바꾸면 이후 수정이 사라짐)
  for (const [k, v] of Object.entries(patch)) if (!LOCAL_KEYS.includes(k)) config[k] = v;
  renderSummary();
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    const p = pending; pending = {};
    try {
      const keep = Object.fromEntries(LOCAL_KEYS.map(k => [k, config[k]]));   // 편집 중인 값은 화면 쪽 객체를 그대로 유지
      config = (await api('set_config', { patch: p })).config;
      for (const [k, v] of Object.entries(keep)) if (v) config[k] = v;
      renderSummary();
    }
    catch { toast('설정 저장 실패'); }
  }, 350);
}

function bindFields() {
  for (const [id, key, type] of FIELDS) {
    const el = $(id);
    const read = () => {
      if (type === 'check') return el.checked;
      if (type === 'lines') return el.value.split('\n').map(s => s.trim()).filter(Boolean);
      if (type === 'int') { const n = parseInt(el.value, 10); return Number.isFinite(n) ? n : config[key]; }
      return el.value.trim();
    };
    el.addEventListener(type === 'check' || el.tagName === 'SELECT' ? 'change' : 'input', () => {
      queueSave({ [key]: read() });
    });
  }
  $('openFolder').addEventListener('click', () => api('open_folder'));
}

// 바이옴 설정 — 템플릿을 눌러 켜고 끄기, 직접 추가도 가능
const BIOME_TEMPLATES = ['Cyberspace', 'Glitched', 'Dreamspace'];
function renderBiomes() {
  const box = $('biomeChips');
  const sel = config.keywords || [];
  const lower = sel.map(s => s.toLowerCase());
  const all = [...BIOME_TEMPLATES, ...sel.filter(s => !BIOME_TEMPLATES.some(t => t.toLowerCase() === s.toLowerCase()))];
  box.innerHTML = '';
  for (const name of all) {
    const on = lower.includes(name.toLowerCase());
    const custom = !BIOME_TEMPLATES.includes(name);
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'chip' + (on ? ' on' : '');
    b.innerHTML = `<span class="mark"></span><span class="nm"></span>${custom ? `<span class="rm" title="삭제">${icon('x')}</span>` : ''}`;
    b.querySelector('.nm').textContent = name;
    b.addEventListener('click', e => {
      let next = (config.keywords || []).filter(s => s.toLowerCase() !== name.toLowerCase());
      if (!(custom && e.target.closest('.rm')) && !on) next.push(name);
      queueSave({ keywords: next });
      renderBiomes();
    });
    box.appendChild(b);
  }
}
$('biomeAdd').addEventListener('submit', e => {
  e.preventDefault();
  const inp = $('biomeAdd').querySelector('input');
  const v = inp.value.trim();
  if (!v) return;
  const cur = config.keywords || [];
  if (!cur.some(s => s.toLowerCase() === v.toLowerCase())) queueSave({ keywords: [...cur, v] });
  inp.value = '';
  renderBiomes();
});

// 감시 대상 목록 — 이름만 보이고, 누르면 아래로 펼쳐지며 상세(서버 / ID) 표시
const openIds = new Set();
function targetInfo(kind, id) {
  if (kind === 'guild') {
    const n = (config.names || {})[id];
    return { title: n, rows: [['ID', id]] };
  }
  const c = (config.channels || {})[id];
  if (c && c.name) {
    const title = c.name.startsWith('#') || c.name.includes('(서버 ID임)') ? c.name : '#' + c.name;
    return { title, rows: [['서버', c.guild || '알 수 없음'], ['ID', id]] };
  }
  const legacy = (config.names || {})[id];          // 예전 형식 "서버 #채널"
  if (legacy) {
    const k = legacy.lastIndexOf(' #');
    if (k >= 0) return { title: legacy.slice(k + 1), rows: [['서버', legacy.slice(0, k)], ['ID', id]] };
    return { title: legacy, rows: [['ID', id]] };
  }
  return { title: null, rows: [['ID', id]] };
}

function renderLists() {
  for (const kind of ['guild', 'channel']) {
    const ul = $('list-' + kind);
    const ids = config[kind + '_ids'] || [];
    ul.innerHTML = ids.length ? '' : '<li class="none">비어 있음</li>';
    for (const id of ids) {
      const info = targetInfo(kind, id);
      const li = document.createElement('li');
      li.className = 'target' + (openIds.has(kind + id) ? ' open' : '');
      li.innerHTML = `<div class="trow"><span class="caret">${icon('chev')}</span><span class="tname"></span>
        <button class="del" title="삭제">${icon('x')}</button></div>
        <div class="tdetail"><div class="tdetail-in"></div></div>`;
      const tn = li.querySelector('.tname');
      tn.textContent = info.title || '이름 모름 · 디스코드 연결 시 가져옴';
      if (!info.title) tn.classList.add('unknown');
      const box = li.querySelector('.tdetail-in');
      for (const [label, value] of info.rows) {
        const row = document.createElement('div');
        row.className = 'tid';
        row.innerHTML = `<span class="k"></span><span class="v"></span>`;
        row.querySelector('.k').textContent = label + ' :';
        row.querySelector('.v').textContent = value;
        if (label === 'ID') {
          row.querySelector('.v').classList.add('mono');
          const cb = document.createElement('button');
          cb.className = 'btn mini ghost copy';
          cb.textContent = '복사';
          cb.addEventListener('click', e => {
            e.stopPropagation();
            navigator.clipboard.writeText(id).then(() => toast('ID 복사됨'), () => toast('복사 실패'));
          });
          row.appendChild(cb);
        }
        box.appendChild(row);
      }
      li.querySelector('.trow').addEventListener('click', () => {
        li.classList.toggle('open');
        li.classList.contains('open') ? openIds.add(kind + id) : openIds.delete(kind + id);
      });
      li.querySelector('.del').addEventListener('click', async e => {
        e.stopPropagation();
        openIds.delete(kind + id);
        config = (await api('remove_target', { kind, id })).config;
        renderLists(); renderSummary();
      });
      ul.appendChild(li);
    }
  }
}

async function addTarget(kind, id, name, guild, guildId) {
  const r = await api('add_target', { kind, id, name, guild, guildId });
  if (r.error) return toast(r.error);
  config = r.config;
  renderLists(); renderSummary();
  toast(r.note || (kind === 'guild' ? '서버 추가됨' : '채널 추가됨'));
  Tutorial.onTargetAdded();
}

document.querySelectorAll('form.add').forEach(f => f.addEventListener('submit', e => {
  e.preventDefault();
  const inp = f.querySelector('input');
  const v = inp.value.trim();
  if (!/^\d+$/.test(v)) return toast('숫자 ID만 입력 가능');
  addTarget(f.dataset.kind, v).then(() => { inp.value = ''; });
}));

// ---------------------------------------------------------------- 감지 기록
const KNOWN = ['열림', '감지', '보임', '대기', '중복', '건너뜀'];
function fmtTime(t) {
  const d = new Date(t * 1000), p = n => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

async function copy(text) {
  try { await navigator.clipboard.writeText(text); }
  catch {
    const ta = document.createElement('textarea');
    ta.value = text; document.body.appendChild(ta); ta.select(); document.execCommand('copy'); ta.remove();
  }
  toast('링크 복사됨');
}

function addEventRow(e) {
  const tr = document.createElement('tr');
  tr.className = 'new';
  const cls = KNOWN.includes(e.status) ? 'st-' + e.status : 'st-other';
  tr.innerHTML = `<td></td><td class="st ${cls}"></td><td></td><td class="url"></td><td class="acts"></td>`;
  const td = tr.children;
  td[0].textContent = fmtTime(e.time);
  td[1].textContent = e.status;
  td[2].textContent = e.where;
  td[3].textContent = e.url;
  td[3].title = e.url;
  const acts = td[4];
  const btn = (label, fn, ghost = true) => {
    const b = document.createElement('button');
    b.className = 'btn mini' + (ghost ? ' ghost' : '');
    b.textContent = label;
    b.addEventListener('click', fn);
    acts.appendChild(b);
  };
  btn('복사', () => copy(e.url));
  btn('열기', () => api('open_url', { url: e.url }));
  const body = $('eventRows');
  body.prepend(tr);
  while (body.children.length > 300) body.lastChild.remove();
  $('eventsEmpty').style.display = 'none';
}

$('clearEvents').addEventListener('click', () => {
  $('eventRows').innerHTML = '';
  $('eventsEmpty').style.display = '';
});

// ---------------------------------------------------------------- 로그
function scrollLog(force) {
  const l = $('log');
  if (force || l.scrollHeight - l.scrollTop - l.clientHeight < 40) l.scrollTop = l.scrollHeight;
}
function addLog(x) {
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

// ---------------------------------------------------------------- 시작 / 중지 (메인 화면 기능별 버튼 3개)
// 바이옴 매크로 = 바이옴 매크로 설정의 켜기 · 오토 스나이핑 = 예전 시작 버튼(감지되면 접속) · 매크로 = 매크로 탭 기능 전체
const runBtn = $('runBtn');
const tile = k => document.querySelector(`.main-tile[data-main="${k}"]`);
function setTile(k, on, btnText) {
  const t = tile(k);
  t.classList.toggle('on', !!on);
  t.querySelector('.mt-go').textContent = btnText || (on ? '끄기' : '시작');
}
function setRun(state) {                  // 오토 스나이핑 (state: idle / running)
  runBtn.dataset.state = state;
  setTile('snipe', state === 'running');
}
runBtn.addEventListener('click', async () => {
  const s = runBtn.dataset.state;
  if (s === 'idle') {
    const r = await api('start');
    if (r.error) {                          // 오토 팝핑 설정을 안 하면 시작 안 됨 → 설정 튜토리얼
      setRun('idle');
      toast(`오토 팝핑 설정을 먼저 끝내야 시작 가능 (${r.missing.length}개 남음)`);
      if (!Tutorial.isOpen()) Tutorial.start('popping');
      return;
    }
    setRun('running');
  }
  else { setRun('idle'); await api('stop'); }
  renderSummary();
});
$('mgBiome').addEventListener('click', () => setBioEnabled(!bio().enabled));
$('mgMacro').addEventListener('click', () => {
  const on = !config.macro_on;
  config.macro_on = on;
  queueSave({ macro_on: on });
  if (!on) { api('mpop_stop'); api('mfish_stop'); }
  syncMainTiles();
  toast('매크로 ' + (on ? '켜짐' : '꺼짐'));
});
document.querySelectorAll('[data-tab-go]').forEach(b => b.addEventListener('click', () => setTab(b.dataset.tabGo, true)));

// 켜진 매크로 탭 기능 수
const macroFeatures = () => [mpop().enabled, mfish().enabled].filter(Boolean).length;
let lastBio = null, lastMpop = null, lastMfish = null;
function syncMainTiles() {
  const bOn = !!bio().enabled;
  setTile('biome', bOn);
  const cur = lastBio && lastBio.current;
  $('mtBiome').textContent = bOn ? `작동 중 · ${cur || '바이옴 확인 중'}`
    : (bio().player ? '꺼짐' : '꺼짐 · 플레이어 이름 필요');
  $('mtSnipe').textContent = armed ? `감시 중 · ${$('count').textContent}개 감지` : '꺼짐';
  const mOn = !!config.macro_on, n = macroFeatures();
  const sniping = !!(lastBio && lastBio.muted);
  setTile('macro', mOn);
  tile('macro').classList.toggle('wait', mOn && sniping);
  $('mtMacro').textContent = !mOn ? (n ? `꺼짐 · 기능 ${n}개 켜짐` : '꺼짐')
    : lastMpop && lastMpop.running ? (lastMpop.msg || '포션 사용 중')
    : lastMfish && lastMfish.running ? (lastMfish.msg || '낚시 중')
    : sniping ? '스나이핑 중이라 대기'
    : n ? `작동 중 · 기능 ${n}개` : '켜진 기능 없음';
  const on = [bOn && '바이옴 매크로', mOn && '매크로', armed && '오토 스나이핑'].filter(Boolean);
  $('modeText').textContent = on.length ? `작동 중: ${on.join(' · ')}` : '모두 꺼짐 — 아래 버튼으로 켜기';
}

function setStatus([state, text]) {
  document.querySelectorAll('[data-status-dot]').forEach(d => { d.dataset.s = state; });
  document.querySelectorAll('[data-status-text]').forEach(t => { t.textContent = text; });
}

// ---------------------------------------------------------------- 주기적으로 엔진 상태 받기
let lastNames = '';
async function poll() {
  try {
    const r = await api('poll', { since: seq });
    seq = r.seq;
    setStatus(r.status);
    $('count').textContent = r.count;
    setRun(r.armed ? 'running' : 'idle');
    if (r.armed !== armed) { armed = r.armed; renderSummary(); }
    r.logs.forEach(addLog);
    updateCrash(r.crash);
    updatePlay(r.play, r.roblox, r.pop);
    updateRet(r.ret, r.play);
    updateSteps(r.steps, r.pre);
    updateBiome(r.biome);
    updateMpop(r.mpop);
    updateMfish(r.mfish);
    lastBio = r.biome; lastMpop = r.mpop; lastMfish = r.mfish;
    syncMainTiles();
    r.events.forEach(addEventRow);
    const nm = JSON.stringify([r.names, r.channels]);
    if (nm !== lastNames) { lastNames = nm; config.names = r.names; config.channels = r.channels; renderLists(); }
  } catch {
    setStatus(['error', '프로그램과 연결 끊김']);
  }
  // 창이 최소화돼 있거나 Play 클릭 중이면 덜 자주 받아옴 (그동안 CPU 를 매크로에 양보)
  setTimeout(poll, document.hidden ? 1500 : (playBusy ? 800 : 300));
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
    if (picking) return;
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
  if (crash) $('crashText').textContent = `${Math.ceil(crash.left)}초 후 매크로 복귀를 실행합니다`;
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

function updatePlay(st, roblox, pst) {
  updateRbxNotice(roblox);
  playBusy = !!(st && st.running) || !!(pst && pst.running);
  if (pst && pst.running) st = { running: true, msg: '오토 팝핑 · ' + (pst.msg || '') };
  $('robloxState').textContent = roblox ? '로블록스 창 감지됨' : '로블록스 창 없음';
  $('robloxState').classList.toggle('ok', !!roblox);
  const running = st && st.running;
  $('playDot').dataset.s = running ? 'flux' : '';
  $('playState').textContent = running ? (st.msg || '실행 중') : '대기';
  $('playTest').disabled = !!running;
}

// ---------------------------------------------------------------- 오토 팝핑 (레어 바이옴 포션)
const POP_POS = [['inventory_pos', 'Inventory 버튼'], ['items_pos', 'Items 버튼'], ['search_pos', 'Search 버튼'],
                 ['item_pos', '아이템 칸', '검색 결과 첫 칸'], ['amount_pos', '수량 입력칸'], ['use_pos', 'Use 버튼']];
const POP_DELAYS = [['after_join', 'Play 후 게임 입장 → 시작', 7.5], ['inventory', 'Inventory 클릭 후', 0.5],
  ['items', 'Items 클릭 후', 0.5], ['search', 'Search 클릭 후', 0.5], ['typing', '아이템 이름 입력 + 엔터 후', 0.5],
  ['item_click', '아이템 클릭 후', 0.35], ['dbl_gap', '수량칸 더블클릭 간격', 0.1], ['after_dbl', '수량칸 더블클릭 후', 0.5],
  ['after_enter', '개수 입력 + 엔터 후', 0.35]];
const POP_BIOMES = ['CYBERSPACE', 'GLITCHED', 'DREAMSPACE'];
const pop = () => (config.pop ||= {});
const savePop = () => queueSave({ pop: JSON.parse(JSON.stringify(pop())) });
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const fmtPos = p => p ? `${pct(p[0])}, ${pct(p[1])}` : '지정 안 됨';
const fmtReg = r => r ? `${pct(r[0])}, ${pct(r[1])} → ${pct(r[2])}, ${pct(r[3])}` : '지정 안 됨';

async function pickWith(btn, label, call) {
  if (picking) return null;
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
            ocr_region: [0.415, 0.392, 0.469, 0.491] },
};
const tplRowHTML = () => `
  <div class="row tpl-pick"><span>위치 템플릿<small>Play/Click to skip, 버튼 위치, OCR 영역을 한 번에 채움 · 로블록스 화면 비율에 맞는 것 선택</small></span>
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

function renderPopSet() {
  const box = $('popSetForm');
  const p = pop();
  box.innerHTML = tplRowHTML() + POP_POS.map(([k, name, sub]) => `
    <div class="row"><span>${name} 위치<small>${sub ? sub + ' · ' : ''}직접 지정 필요</small></span>
      <span class="pos"><code class="${p[k] ? '' : 'unset'}" data-code="${k}">${fmtPos(p[k])}</code>
      <button class="btn mini ghost" type="button" data-pick="${k}">위치 지정</button></span></div>`).join('') + `
    <div class="row"><span>OCR 영역<small>검색 결과 아이템 이름·개수 (예: Warp Potion x23)</small></span>
      <span class="pos"><code class="${p.ocr_region ? '' : 'unset'}" id="popRegion">${fmtReg(p.ocr_region)}</code>
      <button class="btn mini ghost" type="button" id="popRegionPick">드래그로 지정</button>
      <button class="btn mini ghost" type="button" id="popOcrTest">OCR 테스트</button></span></div>
    <label class="row"><span>이름 일치율 기준<small>OCR 이름과 포션 이름이 이 이상 같아야 사용 · 미만이면 1회 재검색 후 스킵 (%)</small></span>
      <input type="number" min="1" max="100" step="5" id="popThreshold" value="${p.match_threshold ?? 70}"></label>`;
  box.querySelectorAll('[data-pick]').forEach(b => b.addEventListener('click', async () => {
    const k = b.dataset.pick, name = POP_POS.find(x => x[0] === k)[1];
    const r = await pickWith(b, `로블록스 화면에서 ${name} 클릭`, () => api('pop_pos', { key: k }));
    if (r) { pop()[k] = r.pos; renderPopSet(); savePop(); toast(`${name} 위치 저장`); Tutorial.refresh(); }
  }));
  $('popRegionPick').addEventListener('click', async e => {
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

function renderPopBiomes() {
  const on = (pop().biomes_on ||= {});
  const box = $('popBiomeForm');
  const label = { CYBERSPACE: 'Cyberspace', GLITCHED: 'Glitched', DREAMSPACE: 'Dreamspace' };
  box.innerHTML = POP_BIOMES.map(b => `
    <label class="row"><span>${label[b]}<small>켜져 있어야 이 바이옴에서 오토 팝핑 · 포션 목록은 ${label[b]} 탭</small></span>
      <span class="switch"><input type="checkbox" data-pop-on="${b}" ${on[b] !== false ? 'checked' : ''}><i></i></span></label>`).join('');
  box.querySelectorAll('[data-pop-on]').forEach(i => i.addEventListener('change', () => {
    on[i.dataset.popOn] = i.checked; savePop();
  }));
}

function fillPop() {
  renderPopSet();
  renderPopBiomes();
  renderPopDelays();
  POP_BIOMES.forEach(b => renderTemplate(b));
}
document.querySelectorAll('[data-pop-default]').forEach(b => b.addEventListener('click', async () => {
  const bio = b.dataset.popDefault;
  const r = await api('pop_default', { biome: bio });
  if (r.error) return toast(r.error);
  (pop().templates ||= {})[bio] = r.template;
  renderTemplate(bio); Tutorial.refresh();
  toast('기본 템플릿으로 되돌림');
}));
document.querySelectorAll('[data-pop-test]').forEach(b => b.addEventListener('click', async () => {
  const r = await api('pop_test', { biome: b.dataset.popTest });
  toast(r.error || '오토 팝핑 테스트 시작 · 인벤토리 열기부터 · 정지: 위쪽 [정지] 또는 F7');
}));

// ---------------------------------------------------------------- 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버)
// 포션 목록 · 켜진 바이옴 · 버튼 위치 · OCR 영역은 따로(mpop), 딜레이 · 일치율은 오토 팝핑(pop) 설정을 같이 씀
const mpop = () => (config.mpop ||= {});
const saveMpop = () => queueSave({ mpop: JSON.parse(JSON.stringify(mpop())) });
const POP_POS_KEYS = ['inventory_pos', 'items_pos', 'search_pos', 'item_pos', 'amount_pos', 'use_pos', 'ocr_region'];
function renderMpop() {
  const m = mpop(), on = (m.biomes_on ||= {});
  const player = ((config.biome || {}).player || '').trim();
  const posMiss = POP_POS_KEYS.filter(k => !m[k]).length;
  const label = { CYBERSPACE: 'Cyberspace', GLITCHED: 'Glitched', DREAMSPACE: 'Dreamspace' };
  $('mpopForm').innerHTML = `
    <label class="row"><span>켜기<small>켜져 있는 동안 레어 바이옴이 감지되면 아래 포션 목록대로 사용 (시작 버튼과 무관)</small></span>
      <span class="switch"><input type="checkbox" id="mpopOn" ${m.enabled ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>플레이어 이름<small>바이옴 감지에 필요 · 바이옴 매크로 설정 → 기본 설정에서 입력</small></span>
      <span class="${player ? '' : 'warn'}">${player ? esc(player) : '입력 안 됨 — 바이옴 감지 안 됨'}</span></div>
    <div class="row"><span>버튼 위치 · OCR 영역<small>매크로 기준 위치 설정 → 레어 바이옴 자동 팝핑 에서 지정</small></span>
      <span class="${posMiss ? 'warn' : ''}">${posMiss ? `${posMiss}개 지정 안 됨` : '지정됨'}</span></div>
    <div class="row"><span>딜레이 · 이름 일치율<small>오토 팝핑 매크로 설정(스나이프 탭)에 지정한 것을 같이 씀</small></span><span>오토 팝핑과 같음</span></div>
    <label class="row"><span>시작 전 대기<small>바이옴 감지 후 인벤토리를 열기까지 (초) · 기본 1</small></span>
      <input type="number" min="0" max="60" step="0.5" id="mpopDelay" value="${m.start_delay ?? 1}"></label>
    <label class="row"><span>다 쓰고 인벤토리 닫기<small>Inventory 버튼을 한 번 더 눌러 닫음</small></span>
      <span class="switch"><input type="checkbox" id="mpopClose" ${m.close_inventory !== false ? 'checked' : ''}><i></i></span></label>` +
    POP_BIOMES.map(b => `
    <label class="row"><span>${label[b]}<small>켜져 있어야 이 바이옴에서 팝핑 · 포션 목록은 아래</small></span>
      <span class="switch"><input type="checkbox" data-mpop-on="${b}" ${on[b] !== false ? 'checked' : ''}><i></i></span></label>`).join('');
  $('mpopOn').addEventListener('change', e => {
    setFeature('mpop', e.target.checked);
  });
  $('mpopDelay').addEventListener('input', e => {
    const n = parseFloat(e.target.value);
    if (n >= 0 && n <= 60) { m.start_delay = n; saveMpop(); }
  });
  $('mpopClose').addEventListener('change', e => { m.close_inventory = e.target.checked; saveMpop(); });
  $('mpopForm').querySelectorAll('[data-mpop-on]').forEach(i => i.addEventListener('change', () => {
    on[i.dataset.mpopOn] = i.checked; saveMpop();
  }));
}
function fillMpop() {
  renderMpop();
  POP_BIOMES.forEach(b => renderTemplate(b, mpop, saveMpop, 'data-mtpl'));
}
function updateMpop(st) {
  const running = !!(st && st.running);
  $('mpopDot').dataset.s = running ? 'flux' : '';
  $('mpopState').textContent = running ? (st.msg || '실행 중') : '대기';
  document.querySelectorAll('[data-mpop-test]').forEach(b => { b.disabled = running; });
}
document.querySelectorAll('[data-mpop-default]').forEach(b => b.addEventListener('click', async () => {
  const bio = b.dataset.mpopDefault;
  const r = await api('mpop_default', { biome: bio });
  if (r.error) return toast(r.error);
  (mpop().templates ||= {})[bio] = r.template;
  renderTemplate(bio, mpop, saveMpop, 'data-mtpl');
  toast('기본 템플릿으로 되돌림');
}));
document.querySelectorAll('[data-mpop-test]').forEach(b => b.addEventListener('click', async () => {
  const r = await api('mpop_test', { biome: b.dataset.mpopTest });
  toast(r.error || '레어 바이옴 자동 팝핑 테스트 시작 · 인벤토리 열기부터 · 정지: 위쪽 [정지] 또는 F7');
}));
$('mpopStop').addEventListener('click', () => api('mpop_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 자동 낚시 (제자리 낚시)
const mfish = () => (config.mfish ||= {});
const saveMfish = () => queueSave({ mfish: JSON.parse(JSON.stringify(mfish())) });
const MFISH_POS = [['fish_btn', 'Fish 버튼', 'Fish / Exit 버튼 가운데 (같은 자리)'], ['close_pos', '결과창 X', '낚시 결과창 오른쪽 위 X'],
  ['title_pos', '결과창 제목', '선택 · 제목 색으로 성공 / 쓰레기 / 실패 구분']];
const MFISH_REQ = ['fish_btn', 'bar_region', 'close_pos'];
const MFISH_TUNE = [['bite_max', '입질 최대 대기', '이 시간 동안 입질이 없으면 Exit 후 다시 던짐 (초)', 60, 5, 1],
  ['target_pct', '목표 위치', '구간 왼쪽 끝에서 구간 폭의 몇 % 지점까지 떨어지면 누를지 · 0 = 왼쪽 끝, 50 = 가운데 · 구간 왼쪽으로 자꾸 빠지면 늘리고, 오른쪽으로 넘어가면 줄임 (%)', 10, 0, 5],
  ['lead_ms', '미리 누르기', '떨어지는 속도를 보고 이만큼 미리 누름 · 구간을 자꾸 넘어가면 늘리고, 못 따라가면 줄임 (ms)', 60, 0, 10],
  ['click_gap_ms', '클릭 최소 간격', '릴링 중 클릭 사이 최소 간격 (ms)', 45, 10, 5],
  ['result_wait', '결과창 대기', '릴링이 끝난 뒤 결과창 X 를 누르기까지 (초)', 0.8, 0, 0.1],
  ['cast_retry', 'Fish 다시 누르기', 'Fish 를 눌러도 반응이 없으면 다시 누르는 횟수 · 넘으면 인벤토리 가득으로 봄', 3, 1, 1]];
function renderMfish() {
  const m = mfish();
  const posMiss = MFISH_REQ.filter(k => !m[k]).length;
  $('mfishForm').innerHTML = `
    <label class="row"><span>켜기<small>매크로 버튼이 켜져 있는 동안 계속 낚시 · 레어 바이옴이 뜨면 잠깐 멈추고 팝핑 후 이어감</small></span>
      <span class="switch"><input type="checkbox" id="mfishOn" ${m.enabled ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>버튼 위치 · 릴링 바 영역<small>매크로 기준 위치 설정 → 자동 낚시 에서 지정</small></span>
      <span class="${posMiss ? 'warn' : ''}">${posMiss ? `${posMiss}개 지정 안 됨` : '지정됨'}</span></div>
    <div class="row"><span>이번 실행 기록<small>성공 · 쓰레기 · 실패 · 인벤토리 가득</small></span><b id="mfishStats">-</b></div>`;
  $('mfishOn').addEventListener('change', e => setFeature('mfish', e.target.checked));
  $('mfishTune').innerHTML = MFISH_TUNE.map(([k, name, sub, def, min, step]) => `
    <label class="row"><span>${name}<small>${sub} · 기본 ${def}</small></span>
      <input type="number" min="${min}" step="${step}" data-mfish-tune="${k}" value="${m[k] ?? def}"></label>`).join('');
  $('mfishTune').querySelectorAll('[data-mfish-tune]').forEach(i => i.addEventListener('input', () => {
    const n = parseFloat(i.value);
    if (Number.isFinite(n) && n >= parseFloat(i.min)) { m[i.dataset.mfishTune] = n; saveMfish(); }
  }));
  if (lastMfish) updateMfish(lastMfish);
}
function updateMfish(st) {
  const running = !!(st && st.running);
  $('mfishDot').dataset.s = running ? 'flux' : '';
  $('mfishState').textContent = running ? (st.msg || '실행 중') : '대기';
  const s = st && st.stats, el = $('mfishStats');
  if (s && el) el.textContent = `${s.success} · ${s.junk} · ${s.fail} · ${s.full}`;
}
$('mfishStop').addEventListener('click', () => api('mfish_stop'));

// ---------------------------------------------------------------- 매크로 기능 설정 · 기능 켜기 · 끄기
// 준비 중인 기능은 자리만 (만들면 key 를 채움)
const MFEATS = [['mpop', '레어 바이옴 자동 팝핑', '내 서버에서 레어 바이옴이 뜨면 포션 사용'],
  ['mfish', '자동 낚시', '제자리 낚시 (판매와 이동은 다음 업데이트)'],
  [null, '상인 자동 구매', '준비 중'], [null, '포션 자동 제작', '준비 중'], [null, '오토 메모리 매치', '준비 중']];
const FEAT_NAME = { mpop: '레어 바이옴 자동 팝핑', mfish: '자동 낚시' };
const featCfg = k => ({ mpop, mfish })[k]();
function setFeature(k, on, quiet) {
  const c = featCfg(k);
  c.enabled = !!on;
  queueSave({ [k]: JSON.parse(JSON.stringify(c)) });
  if (k === 'mfish' && !on) api('mfish_stop');
  if (k === 'mpop') renderMpop(); else renderMfish();
  renderMfAll(); syncMainTiles();
  if (!quiet) toast(`${FEAT_NAME[k]} ${on ? '켜짐' : '꺼짐'}`);
}
function renderMfAll() {
  $('mfAllForm').innerHTML = MFEATS.map(([k, name, sub]) => k ? `
    <label class="row"><span>${name}<small>${sub}</small></span>
      <span class="switch"><input type="checkbox" data-feat="${k}" ${featCfg(k).enabled ? 'checked' : ''}><i></i></span></label>` : `
    <div class="row feat-soon"><span>${name}<small>${sub}</small></span>
      <span class="switch"><input type="checkbox" disabled><i></i></span></div>`).join('');
  $('mfAllForm').querySelectorAll('[data-feat]').forEach(i => i.addEventListener('change', () => setFeature(i.dataset.feat, i.checked)));
}
function setAllFeatures(on) {
  MFEATS.forEach(([k]) => k && setFeature(k, on, true));
  toast(on ? '기능 전부 켜짐' : '기능 전부 꺼짐');
}
$('mfAllOn').addEventListener('click', () => setAllFeatures(true));
$('mfAllOff').addEventListener('click', () => setAllFeatures(false));

// ---------------------------------------------------------------- 매크로 기준 위치 설정 (기능마다 따로)
// feat: 설정 묶음 (mpop / mfish) · points: [키, 이름, 설명] · region: [키, 이름, 설명] · 저장은 서버(api)에서
const MPOS = {
  mpop: { box: 'mposPop', points: POP_POS.map(([k, n, sub]) => [k, n, sub || '']),
          region: ['ocr_region', 'OCR 영역', '검색 결과 아이템 이름·개수 (예: Warp Potion x23)'] },
  mfish: { box: 'mposFish', points: MFISH_POS,
           region: ['bar_region', '릴링 바 영역', '위쪽 바(파란 막대, Ready! 가 뜨는 바)만 딱 맞게 드래그 · ◇ 표시는 자동으로 찾음'] },
};
const MPOS_RATIOS = [['auto', '자동 (지금 창)'], ['16:9', '16:9'], ['16:10', '16:10'], ['21:9', '21:9'], ['32:9', '32:9'], ['4:3', '4:3'], ['5:4', '5:4']];
let mposRatio = 'auto';
function renderMpos(feat) {
  const d = MPOS[feat], c = featCfg(feat), box = $(d.box);
  const [rk, rname, rsub] = d.region;
  const row = (k, name, sub, val, fmt, btn, attr) => `
    <div class="row"><span>${name}<small>${sub}</small></span>
      <span class="pos"><code class="${val ? '' : 'unset'}">${fmt(val)}</code>
      <button class="btn mini ghost" type="button" ${attr}="${k}">${btn}</button></span></div>`;
  box.innerHTML = `
    <div class="row tpl-pick"><span>위치 템플릿<small>화면 비율에 맞는 기본 위치를 한 번에 채움 · 자동 = 지금 로블록스 창 크기로 계산 · 16:9 말고는 추정값이라 안 맞는 건 아래에서 직접 지정</small></span>
      <span class="pos"><select data-mpos-ratio>${MPOS_RATIOS.map(([v, n]) => `<option value="${v}">${n}</option>`).join('')}</select>
      <button class="btn mini" type="button" data-mpos-tpl>적용</button></span></div>` +
    d.points.map(([k, name, sub]) => row(k, `${name} 위치`, sub, c[k], fmtPos, '위치 지정', 'data-mpos-pick')).join('') +
    row(rk, rname, rsub, c[rk], fmtReg, '드래그로 지정', 'data-mpos-region');
  const sel = box.querySelector('[data-mpos-ratio]');
  sel.value = mposRatio;
  sel.addEventListener('change', () => { mposRatio = sel.value; });
  box.querySelector('[data-mpos-tpl]').addEventListener('click', async e => {
    const b = e.currentTarget;
    const set = [...d.points.map(x => x[0]), rk].some(k => c[k]);
    if (set && !(b._armed > Date.now())) {             // 이미 지정한 게 있으면 한 번 더 눌러야 덮어씀
      b._armed = Date.now() + 3000; b.textContent = '한 번 더 누르면 덮어쓰기';
      setTimeout(() => { b.textContent = '적용'; b._armed = 0; }, 3000);
      return;
    }
    const ratio = box.querySelector('[data-mpos-ratio]').value;
    mposRatio = ratio;
    const r = await api('mpos_template', { feat, ratio });
    if (r.error) return toast(r.error);
    Object.assign(c, r[feat]); mposChanged(feat);
    toast(r.guess ? `${r.label} 템플릿 적용 · 추정값이라 [상태 확인] 으로 확인` : `${r.label} 템플릿 적용`);
  });
  box.querySelectorAll('[data-mpos-pick]').forEach(b => b.addEventListener('click', async () => {
    const k = b.dataset.mposPick, name = d.points.find(x => x[0] === k)[1];
    const r = await pickWith(b, `로블록스 화면에서 ${name} 클릭`, () => api('mpos_point', { feat, key: k }));
    if (r) { c[k] = r.pos; mposChanged(feat); toast(`${name} 위치 저장`); }
  }));
  box.querySelector('[data-mpos-region]').addEventListener('click', async e => {
    const r = await pickWith(e.currentTarget, `로블록스 화면에서 ${rname} 드래그`, () => api('mpos_region', { feat }));
    if (r) { c[rk] = r.region; mposChanged(feat); toast(`${rname} 저장`); }
  });
}
// 위치는 서버가 이미 저장함 → 화면만 다시 그림
function mposChanged(feat) {
  renderMpos(feat);
  if (feat === 'mpop') renderMpop(); else renderMfish();
}
$('mposPopCopy').addEventListener('click', async () => {
  const r = await api('mpos_copy_pop');
  if (r.error) return toast(r.error);
  Object.assign(mpop(), r.mpop); mposChanged('mpop'); toast('스나이프 탭 오토 팝핑 위치를 가져옴');
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
    toast([r.button && `버튼: ${r.button}`, r.bar && `릴링 바: ${r.bar}`, r.title && `결과창: ${r.title}`].filter(Boolean).join(' · ') || '지정된 위치 없음');
  } finally { b.disabled = false; }
});

// ---------------------------------------------------------------- Acrux 설정 (OCR 감지 방식 · 일반)
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
}
$('acOcr').addEventListener('change', e => {
  config.ocr_engine = e.target.value;
  queueSave({ ocr_engine: e.target.value });
  setTimeout(refreshOcrInfo, 600);             // 저장된 뒤 다시 확인
});
$('acLang').addEventListener('change', e => setLang(e.target.value));
$('acFolder').addEventListener('click', async () => {
  try { await api('open_folder'); } catch { toast('폴더를 열 수 없음'); }
});

// ---------------------------------------------------------------- 스나이핑 안정성
const snipe = () => (config.snipe ||= {});
const saveSnipe = () => queueSave({ snipe: JSON.parse(JSON.stringify(snipe())) });
function fillSnipe() {
  const s = snipe();
  $('snDelayMin').value = s.delay_min ?? 2;
  $('snDelayMax').value = s.delay_max ?? 4;
  $('snDirect').checked = s.direct !== false;
  $('snSameMin').value = s.same_link_min ?? 10;
  $('snCooldown').value = s.cooldown_sec ?? 5;
}
for (const [id, key, lo, hi, int] of [['snDelayMin', 'delay_min', 0, 60], ['snDelayMax', 'delay_max', 0, 120],
  ['snSameMin', 'same_link_min', 0, 1440, true], ['snCooldown', 'cooldown_sec', 0, 600]]) {
  $(id).addEventListener('input', e => {
    const n = int ? parseInt(e.target.value, 10) : parseFloat(e.target.value);
    if (Number.isFinite(n) && n >= lo && n <= hi) { snipe()[key] = n; saveSnipe(); }
  });
}
// 최소가 최대보다 크면 최대를 최소에 맞춤 (입력이 끝났을 때)
for (const id of ['snDelayMin', 'snDelayMax']) $(id).addEventListener('change', () => {
  const s = snipe();
  if ((s.delay_max ?? 4) < (s.delay_min ?? 2)) { s.delay_max = s.delay_min; saveSnipe(); }
  fillSnipe();
});
$('snDirect').addEventListener('change', e => { snipe().direct = e.target.checked; saveSnipe(); });

// ---------------------------------------------------------------- 매크로 복귀
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
$('retStop').addEventListener('click', () => api('play_stop'));
function updateRet(st, pst) {
  if (!(st && st.running) && window._stepsState?.running) st = { running: true, msg: '복귀 후 동작 · ' + window._stepsState.msg };
  const run = st && st.running;
  const playing = pst && pst.running && /복귀/.test(pst.msg || '') ;
  $('retDot').dataset.s = run ? 'flux' : '';
  $('retState').textContent = run ? (st.msg || '복귀 중') : '대기';
  $('retTest').disabled = !!run;
}

// ---------------------------------------------------------------- 직접 만드는 매크로 (동작 목록 편집기)
// 같은 편집기를 두 곳에서 씀: 오토 팝핑 → 접속 전 동작 / 매크로 복귀 → 복귀 후 동작
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
          add(field('글자 (한 번에 붙여넣기)', text(s, 'text', '입력할 글자'), true),
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

const retSteps = () => (ret().steps ||= []);
const preSteps = () => (play().pre_steps ||= []);
let retEditor, preEditor;
async function fillSteps() {
  try { KEY_LIST = (await api('steps_info')).keys; } catch { KEY_LIST = ['e', '1', '2', '3']; }
  const all = Object.keys(STEP_DEFS);
  retEditor = makeStepEditor({ list: retSteps, save: saveRet, addBar: $('addBar'), listEl: $('steps'), emptyEl: $('stepsEmpty'), types: all });
  preEditor = makeStepEditor({ list: preSteps, save: savePlay, addBar: $('preAddBar'), listEl: $('preSteps'), emptyEl: $('preEmpty'),
    types: ['kill', 'key', 'combo', 'wait', 'run', 'focus', 'text', 'click'] });
  for (const ed of [retEditor, preEditor]) { ed.renderAdd(); ed.render(); }
}
$('stepsRun').addEventListener('click', async () => {
  const r = await api('steps_run', { which: 'ret' });
  toast(r.error || '2초 뒤 실행 · F7 로 정지');
});
$('preRun').addEventListener('click', async () => {
  const r = await api('steps_run', { which: 'pre' });
  toast(r.error || '2초 뒤 실행 · F7 로 정지');
});
function updateSteps(st, pre) {
  window._stepsState = st;
  $('stepsRun').disabled = !!(st && st.running);
  $('preRun').disabled = !!(pre && pre.running);
  retEditor?.setRunning(st);
  preEditor?.setRunning(pre);
}

// ---------------------------------------------------------------- 바이옴 매크로
const bio = () => (config.biome ||= { enabled: false, player: '', ps_link: '', webhooks: ['', ''], everyone: {}, roles: {} });
const BIOME_LIST = ['WINDY', 'SNOWY', 'RAINY', 'SANDSTORM', 'HELL', 'STARFALL', 'HEAVEN', 'CORRUPTION', 'NULL',
                    'GLITCHED', 'DREAMSPACE', 'CYBERSPACE', 'SINGULARITY'];
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
    row.querySelector('b').textContent = name;
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
$('bioPlayer').addEventListener('input', e => { bio().player = e.target.value.trim(); saveBio(); });
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
    toast(r.error || (r.ok === r.total ? `전송 완료 (${r.ok}/${r.total})` : `일부 실패 (${r.ok}/${r.total}) · ${r.errors[0] || ''}`));
  } finally { btn.disabled = false; btn.textContent = '테스트 전송'; }
});

let lastBioHist = '';
function updateBiome(st) {
  if (!st) return;
  const cur = st.current;
  $('biomeNow').textContent = cur ? `바이옴: ${cur}` : '바이옴 인식 안 됨';
  $('biomeDot').dataset.s = cur ? (RARE.includes(cur) ? 'rare' : 'flux') : '';
  const b = bio();
  $('bioStatus').textContent = st.owner === 'noname' ? '플레이어 이름 입력 필요 · 이름이 없으면 감지 안 함'
    : !st.log ? '로블록스 로그 파일 없음 · 로블록스를 한 번 실행하면 생김'
    : st.owner === 'other' ? '닉네임 불일치 · 게임 속 실제 닉네임과 입력한 이름이 다름'
    : st.owner !== 'ok' ? '계정 확인 중 · 게임에 들어가면 닉네임 확인'
    : cur ? `인식 중 · 현재 바이옴 ${cur}` : '로그 확인 중 · 게임에 들어가면 인식';
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
      row.children[1].textContent = name;
      box.appendChild(row);
    }
  }
}

// ---------------------------------------------------------------- 감시 대상 설정 튜토리얼
// 안내 단계는 [다음]으로, 조작 단계는 사용자가 강조된 곳을 직접 눌러야 넘어감
const Tutorial = (() => {
  const el = $('tut'), card = $('tutCard'), spot = $('tutSpot');
  let i = -1, focusEl = null, moved = false, chain = false, skipCount = 0;
  const hasTargets = () => (config.guild_ids || []).length + (config.channel_ids || []).length > 0;
  const discordPage = () => $('page-discord');
  const biomePage = () => $('page-biome');
  const sectionIs = (sec, page = discordPage()) => page.querySelector('.side .item.active')?.dataset.sec === sec;
  const biomeSec = sec => current === 'biome' && sectionIs(sec, biomePage());
  const needsBiome = () => !(config.biome?.player) && !(config.tutorials_done || []).includes('biome');
  const popPage = () => $('page-popping');
  const popSec = sec => current === 'popping' && sectionIs(sec, popPage());
  const popMissing = () => !config.play?.pos || !config.play?.skip_pos || !config.pop?.ocr_region ||
    POP_POS.some(([k]) => !config.pop?.[k]);
  const needsPop = () => popMissing() && !(config.tutorials_done || []).includes('popping');

  // 튜토리얼 목록 — 새 튜토리얼은 여기에 추가 (id 는 완료 기록용)
  const TUTORIALS = [];
  const TARGET_STEPS = [
    { title: '감시 대상 설정이 필요합니다',
      body: `<p>링크를 감지할 <b>디스코드 채널</b> 또는 <b>서버</b>를 먼저 등록해야 작동합니다.</p>
             <p class="dim">등록한 곳에 올라온 비공개 서버 링크만 감지합니다.</p>` },
    { title: '디스코드 개발자 모드를 켜주세요',
      body: `<ol><li>디스코드 왼쪽 아래 <b>톱니바퀴 (사용자 설정)</b>를 눌러주세요.</li>
             <li>왼쪽 목록에서 <b>고급</b>을 눌러주세요.</li>
             <li><b>개발자 모드</b>를 켜주세요.</li></ol>
             <p class="dim">ID 복사 메뉴를 보기 위해 필요하며, 한 번만 하면 됩니다.</p>` },
    { title: 'ID를 복사해주세요',
      body: `<p><b>채널 하나만</b> 감시하려면 링크가 올라오는 채널 이름을 <b>우클릭 → ID 복사하기</b>를 눌러주세요.</p>
             <p><b>서버 전체</b>를 감시하려면 왼쪽 서버 아이콘을 <b>우클릭 → ID 복사하기</b>를 눌러주세요.</p>
             <p class="dim">스레드에 올라온 링크는 감지하지 않습니다.</p>` },
    { title: '디스코드 감지 설정을 눌러주세요',
      body: `<p>강조된 <b>디스코드 감지 설정</b> 버튼을 직접 눌러주세요.</p>`,
      target: () => document.querySelector('.menu-card[data-key="discord"]'),
      done: () => current === 'discord' },
    { title: '감시 대상을 눌러주세요',
      body: `<p>왼쪽 목록에서 강조된 <b>감시 대상</b>을 눌러주세요.</p>`,
      target: () => discordPage().querySelector('.side .item[data-sec="targets"]'),
      done: () => current === 'discord' && sectionIs('targets'),
      needs: 3 },
    { title: 'ID를 붙여넣고 추가해주세요',
      body: `<p>복사한 채널 ID를 이 칸에 붙여넣고 <b>추가</b>를 눌러주세요.</p>
             <p class="dim">서버 ID는 왼쪽 서버 칸에 넣어주세요. 이름은 디스코드에서 자동으로 가져옵니다.</p>`,
      target: () => discordPage().querySelector('form.add[data-kind="channel"]'),
      done: () => hasTargets(),
      needs: 4 },
    { title: '설정이 완료되었습니다',
      body: `<p>감시 대상 등록이 끝났습니다.</p>
             <p>메인 화면 <b>오토 스나이핑</b>의 <b>시작</b> 버튼을 누르면 작동합니다.</p>
             <p class="dim">필터 탭에서 바이옴을 선택하면 원하는 바이옴만 감지합니다.</p>` },
  ];
  TUTORIALS.push({ id: 'targets', name: '감시 대상 설정', steps: TARGET_STEPS, skipAll: true, menu: 'discord',
    needed: () => !hasTargets(),
    skipMsg: '감시 대상 설정 튜토리얼을 건너뛰었습니다 · 감시 대상 탭에서 다시 볼 수 있습니다' });

  // 바이옴 매크로 설정 — 플레이어 이름은 필수, 브섭 링크·웹후크는 선택 (그 단계만 건너뛰기 가능)
  // 단계 옵션: input = 강조된 칸에 입력 후 [다음] / requires = 조건을 채워야 [다음] 가능
  //           optional = [이 단계 건너뛰기] 표시 / skipTo = 건너뛸 때 갈 단계
  const BIOME_STEPS = [
    { title: '바이옴 매크로 설정이 필요합니다',
      body: `<p>로블록스 로그로 현재 바이옴을 인식하고, 바이옴이 바뀌면 <b>디스코드 웹후크</b>로 알림을 보냅니다.</p>
             <p><b>플레이어 이름</b>은 꼭 입력해야 하며, <b>브섭 링크</b>와 <b>웹후크</b>는 선택입니다.</p>`,
      later: true },
    { title: '바이옴 매크로 설정을 눌러주세요',
      body: `<p>강조된 <b>바이옴 매크로 설정</b> 버튼을 직접 눌러주세요.</p>`,
      target: () => document.querySelector('.menu-card[data-key="biome"]'),
      done: () => current === 'biome' },
    { title: '기본 설정을 눌러주세요',
      body: `<p>왼쪽 목록에서 강조된 <b>기본 설정</b>을 눌러주세요.</p>`,
      target: () => biomePage().querySelector('.side .item[data-sec="bio-basic"]'),
      done: () => biomeSec('bio-basic'),
      needs: 1 },
    { title: '플레이어 이름을 입력해주세요',
      body: `<p>로블록스 <b>실제 닉네임</b>을 입력해주세요. 디스플레이 이름이 아닌 <b>@ 뒤의 이름</b>입니다.</p>
             <p class="dim">게임 속 닉네임과 같아야 바이옴을 감지합니다. 필수 항목입니다.</p>`,
      target: () => $('bioPlayer').closest('.row'),
      input: true, requires: () => !!bio().player,
      needs: 2 },
    { title: '브섭 링크를 입력해주세요',
      body: `<p>바이옴 알림에 함께 표시할 <b>비공개 서버 링크</b>를 붙여넣어주세요.</p>
             <p class="dim">선택 항목입니다. 필요 없으면 이 단계를 건너뛰어주세요.</p>`,
      target: () => $('bioPs').closest('.row'),
      input: true, optional: true,
      needs: 2 },
    { title: '디스코드 웹후크를 눌러주세요',
      body: `<p>알림을 디스코드로 받으려면 왼쪽 목록에서 강조된 <b>디스코드 웹후크</b>를 눌러주세요.</p>
             <p class="dim">선택 항목입니다. 알림이 필요 없으면 이 단계를 건너뛰어주세요.</p>`,
      target: () => biomePage().querySelector('.side .item[data-sec="bio-hooks"]'),
      done: () => biomeSec('bio-hooks'),
      optional: true, skipTo: 7,
      needs: 1 },
    { title: '웹후크 주소를 입력해주세요',
      body: `<ol><li>알림을 받을 디스코드 채널의 <b>톱니바퀴 (채널 편집)</b>를 눌러주세요.</li>
             <li><b>연동 → 웹후크 → 새 웹후크</b>를 누른 뒤 <b>웹후크 URL 복사</b>를 눌러주세요.</li>
             <li>복사한 주소를 <b>웹후크 1</b> 칸에 붙여넣어주세요.</li></ol>
             <p class="dim">튜토리얼이 끝난 뒤 위쪽 [테스트 전송]으로 확인할 수 있습니다. 선택 항목입니다.</p>`,
      target: () => biomePage().querySelector('.sec[data-sec="bio-hooks"] .card.form'),
      input: true, optional: true,
      needs: 5 },
    { title: '설정이 완료되었습니다',
      body: `<p>바이옴 매크로 설정이 끝났습니다.</p>
             <p>메인 화면 <b>바이옴 매크로</b>의 <b>시작</b> 버튼이나, 설정 화면 위쪽 스위치를 켜면 알림을 보냅니다.</p>
             <p class="dim">프로그램을 켤 때마다 꺼진 상태로 시작합니다.</p>` },
  ];
  TUTORIALS.unshift({ id: 'biome', name: '바이옴 매크로 설정', steps: BIOME_STEPS, skipAll: false, menu: 'biome',
    needed: () => needsBiome(),
    skipMsg: '바이옴 매크로 설정 튜토리얼은 다음에 다시 표시됩니다' });


  // 오토 팝핑 매크로 설정 — 위치·OCR 영역은 필수, 딜레이·바이옴 템플릿은 선택
  const pickRow = sel => () => document.querySelector(sel)?.closest('.row');
  // 받침 있으면 '을', 없으면 '를' (영어는 '를')
  const eul = w => { const c = w.charCodeAt(w.length - 1); return c >= 0xAC00 && c <= 0xD7A3 && (c - 0xAC00) % 28 ? '을' : '를'; };
  const secStep = (page, sec, label, extra = {}) => ({
    title: `${label}${eul(label)} 눌러주세요`,
    body: `<p>왼쪽 목록에서 강조된 <b>${label}</b>${eul(label)} 눌러주세요.</p>`,
    target: () => page().querySelector(`.side .item[data-sec="${sec}"]`), ...extra });
  const POP_STEPS = [
    { title: '오토 팝핑 설정이 필요합니다',
      body: `<p>스나이핑으로 접속한 뒤 <b>Play</b> 버튼을 누르고, 레어 바이옴이면 <b>포션을 자동으로 사용</b>합니다.</p>
             <p>버튼 위치와 OCR 영역은 꼭 지정해야 하며, 접속 전 동작 · 팝핑 바이옴 · 딜레이 · 템플릿은 선택입니다.</p>
             <p class="dim">위치를 지정할 때는 로블록스를 켜두고, 해당 버튼이 보이는 화면에서 진행해주세요.</p>`,
      later: true },
    { id: 'card', title: '오토 팝핑 매크로 설정을 눌러주세요',
      body: `<p>강조된 <b>오토 팝핑 매크로 설정</b> 버튼을 직접 눌러주세요.</p>`,
      target: () => document.querySelector('.menu-card[data-key="popping"]'),
      done: () => current === 'popping' },
    // 접속 전 동작 (선택)
    { id: 'preSide', ...secStep(popPage, 'pre', '접속 전 동작'),
      body: `<p>게임에 접속하기 직전에 할 동작을 정합니다. 왼쪽 목록에서 강조된 <b>접속 전 동작</b>을 눌러주세요.</p>
             <p class="dim">선택 항목입니다. 필요 없으면 이 단계를 건너뛰어주세요.</p>`,
      done: () => popSec('pre'), optional: true, skipTo: 'playSide', needs: 'card' },
    { title: '접속 전 동작을 추가해주세요',
      body: `<p>예: <b>+ 프로그램 강제 종료</b>로 방해되는 프로그램을 끄거나, <b>+ 키 입력 / + 키 조합</b>으로 단축키를 누를 수 있습니다.</p>
             <p class="dim">위에서부터 순서대로 실행한 뒤 접속합니다. 선택 항목입니다.</p>`,
      target: () => popPage().querySelector('.sec[data-sec="pre"]'), input: true, optional: true, needs: 'preSide' },
    // 게임 접속 (필수)
    { id: 'playSide', ...secStep(popPage, 'play', '게임 접속'), done: () => popSec('play'), needs: 'card' },
    { title: '위치 템플릿을 쓸 수 있습니다',
      body: `<p>로블록스를 <b>16:9</b> 화면(전체 화면 등)으로 쓴다면 <b>적용</b>을 눌러 버튼 위치와 OCR 영역을 한 번에 채울 수 있습니다.</p>
             <p class="dim">선택 항목입니다. 채워진 위치 단계는 건너뜁니다. 화면 비율이 다르면 직접 지정해주세요.</p>`,
      target: () => $('playTplRow'), input: true, optional: true, needs: 'playSide' },
    { title: 'Click to skip 버튼 위치를 지정해주세요',
      body: `<p><b>위치 지정</b>을 누르면 로블록스 화면이 앞으로 나옵니다. 로블록스의 <b>Click to skip</b> 버튼을 한 번 클릭해주세요.</p>
             <p class="dim">Play 버튼과 번갈아 클릭합니다. 필수 항목입니다. Esc 로 취소할 수 있습니다.</p>`,
      target: pickRow('#skipPick'), input: true, requires: () => !!play().skip_pos, done: () => !!play().skip_pos, needs: 'playSide', gif: 'skip' },
    { title: 'Play 버튼 위치를 지정해주세요',
      body: `<p>같은 방법으로 <b>Play</b> 버튼 위치를 지정해주세요.</p>
             <p class="dim">필수 항목입니다.</p>`,
      target: pickRow('#playPick'), input: true, requires: () => !!play().pos, done: () => !!play().pos, needs: 'playSide', gif: 'play' },
    { title: '클릭 간격과 대기 시간을 확인해주세요',
      body: `<p><b>클릭 간격</b>: Play ↔ Click to skip 번갈아 누르는 간격 (기본 0.15초)</p>
             <p><b>접속 후 대기</b>: 로블록스 창이 뜬 뒤 첫 클릭까지 · <b>최대 시도 시간</b>: 이 안에 입장이 안 되면 중지</p>
             <p class="dim">선택 항목입니다. 기본값 그대로 써도 됩니다.</p>`,
      target: () => $('playInterval')?.closest('.card'), input: true, optional: true, needs: 'playSide' },
    // 오토 팝핑 설정 (필수)
    { id: 'setSide', ...secStep(popPage, 'pop-set', '오토 팝핑 설정'), done: () => popSec('pop-set'), needs: 'card' },
    ...POP_POS.map(([k, name, sub]) => ({
      title: `${name} 위치를 지정해주세요`,
      body: `<p><b>위치 지정</b>을 누른 뒤 로블록스 화면에서 <b>${name}</b>${sub ? ` (${sub})` : ''}을 한 번 클릭해주세요.</p>
             <p class="dim">필수 항목입니다.</p>`,
      target: pickRow(`[data-pick="${k}"]`), input: true, requires: () => !!pop()[k], done: () => !!pop()[k], needs: 'setSide',
      gif: { inventory_pos: 'inventory', items_pos: 'items', search_pos: 'search', item_pos: 'item', amount_pos: 'amount', use_pos: 'use' }[k] })),
    { title: 'OCR 영역을 지정해주세요',
      body: `<p><b>드래그로 지정</b>을 누른 뒤, 검색 결과 첫 칸의 <b>아이템 이름과 개수</b>(예: Warp Potion x23)가 들어가도록 드래그해주세요.</p>
             <p class="dim">필수 항목입니다. <b>OCR 테스트</b>로 제대로 읽히는지 확인할 수 있습니다.</p>`,
      target: pickRow('#popRegionPick'), input: true, requires: () => !!pop().ocr_region, done: () => !!pop().ocr_region, needs: 'setSide', gif: 'ocr' },
    // 팝핑 바이옴 설정 (선택)
    { id: 'bioSide', ...secStep(popPage, 'pop-biomes', '팝핑 바이옴 설정'),
      body: `<p>오토 팝핑을 할 바이옴을 고릅니다. 왼쪽 목록에서 강조된 <b>팝핑 바이옴 설정</b>을 눌러주세요.</p>
             <p class="dim">선택 항목입니다. 기본은 세 바이옴 모두 켜져 있습니다.</p>`,
      done: () => popSec('pop-biomes'), optional: true, skipTo: 'delaySide', needs: 'card' },
    { title: '팝핑할 바이옴을 켜고 꺼주세요',
      body: `<p>켜진 바이옴에서만 오토 팝핑을 합니다. 꺼진 바이옴에 들어가면 팝핑 없이 바로 매크로 복귀를 합니다.</p>`,
      target: () => $('popBiomeForm'), input: true, optional: true, needs: 'bioSide' },
    // 딜레이 (선택)
    { id: 'delaySide', ...secStep(popPage, 'pop-delay', '오토 팝핑 딜레이 설정'),
      body: `<p>동작 사이 대기 시간을 바꾸려면 왼쪽 목록에서 강조된 <b>오토 팝핑 딜레이 설정</b>을 눌러주세요.</p>
             <p class="dim">선택 항목입니다. 기본값 그대로 써도 됩니다.</p>`,
      done: () => popSec('pop-delay'), optional: true, skipTo: 'tplSide', needs: 'card' },
    { title: '딜레이를 조정해주세요',
      body: `<p>필요한 항목의 시간(초)을 바꿔주세요. 바꾼 값은 바로 저장됩니다.</p>`,
      target: () => $('popDelayForm'), input: true, optional: true, needs: 'delaySide' },
    // 템플릿 (선택)
    { id: 'tplSide', ...secStep(popPage, 'pop-glitch', 'Glitched'),
      body: `<p>바이옴별로 사용할 포션 목록을 확인합니다. 왼쪽 목록에서 강조된 <b>Glitched</b>를 눌러주세요.</p>
             <p class="dim">선택 항목입니다. 템플릿이 없는 바이옴에서는 오토 팝핑을 하지 않습니다.</p>`,
      done: () => popSec('pop-glitch'), optional: true, skipTo: 'done', needs: 'card' },
    { title: '사용할 포션을 확인해주세요',
      body: `<p>기본 템플릿(레어 바이옴 자동 팝핑)이 미리 들어 있습니다. <b>+ 포션 추가</b>로 추가하고, <b>✕</b>로 삭제할 수 있습니다.</p>
             <p class="dim">위쪽일수록 먼저 사용합니다. Cyberspace · Dreamspace 도 같은 방법으로 바꿀 수 있습니다. 선택 항목입니다.</p>`,
      target: () => document.querySelector('.tpl[data-tpl="GLITCHED"]'), input: true, optional: true, needs: 'tplSide' },
    { id: 'done', title: '설정이 완료되었습니다',
      body: `<p>오토 팝핑 설정이 끝났습니다.</p>
             <p>링크로 접속하면 Play 버튼을 누르고, 켜진 레어 바이옴이면 포션을 자동으로 사용합니다.</p>
             <p class="dim">각 바이옴 탭의 <b>이 템플릿 테스트</b>로 바로 확인할 수 있습니다. 정지는 F7 입니다.</p>` },
  ];
  TUTORIALS.push({ id: 'popping', name: '오토 팝핑 설정', steps: POP_STEPS, skipAll: true, skipTimes: 3, menu: 'popping',
    needed: () => needsPop(),
    skipMsg: '오토 팝핑 설정 튜토리얼을 건너뛰었습니다 · 설정을 끝내기 전까지 매크로는 시작되지 않습니다' });

  // 매크로 복귀 설정 — 내 브섭 링크는 필수, 복귀 후 동작은 선택
  const retPage = () => $('page-return');
  const retSec = sec => current === 'return' && sectionIs(sec, retPage());
  const needsRet = () => !(config.ret?.ps_link) && !(config.tutorials_done || []).includes('return');
  const RET_STEPS = [
    { title: '매크로 복귀 설정이 필요합니다',
      body: `<p>바이옴이 끝나거나 오토 팝핑이 어떤 이유로든 끝나면, <b>로블록스를 모두 종료</b>하고 <b>내 서버로 돌아갑니다</b>.</p>
             <p><b>내 브섭 링크</b>는 꼭 입력해야 하며, <b>복귀 후 동작</b>은 선택입니다.</p>`,
      later: true },
    { id: 'card', title: '매크로 복귀 설정을 눌러주세요',
      body: `<p>강조된 <b>매크로 복귀 설정</b> 버튼을 직접 눌러주세요.</p>`,
      target: () => document.querySelector('.menu-card[data-key="return"]'),
      done: () => current === 'return' },
    { id: 'setSide', ...secStep(retPage, 'ret-set', '복귀 설정'), done: () => retSec('ret-set'), needs: 'card' },
    { title: '내 브섭 링크를 입력해주세요',
      body: `<p>돌아갈 <b>내 비공개 서버 링크</b>를 붙여넣어주세요.</p>
             <p class="dim">바이옴 매크로 설정에 이미 넣었다면 <b>가져오기</b>로 그대로 쓸 수 있습니다. 필수 항목입니다.</p>`,
      target: () => $('retLink')?.closest('.card'), input: true, requires: () => !!ret().ps_link, needs: 'setSide' },
    { id: 'stepsSide', ...secStep(retPage, 'ret-steps', '복귀 후 동작'),
      body: `<p>내 서버에 들어간 뒤 할 동작을 정합니다. 왼쪽 목록에서 강조된 <b>복귀 후 동작</b>을 눌러주세요.</p>
             <p class="dim">선택 항목입니다. 필요 없으면 이 단계를 건너뛰어주세요.</p>`,
      done: () => retSec('ret-steps'), optional: true, skipTo: 'done', needs: 'card' },
    { title: '복귀 후 동작을 추가해주세요',
      body: `<p>키 입력 · 키 조합(Ctrl+L 등) · 클릭 · 프로그램 실행 · 창 맨 앞으로 등을 순서대로 추가할 수 있습니다.</p>
             <p class="dim">게임 입장이 감지되고 <b>입장 후 대기</b>(기본 7.5초) 뒤에 시작합니다. 키 입력은 맨 앞에 있는 창으로 들어가니, 필요하면 <b>창 맨 앞으로</b>를 먼저 넣어주세요. 왼쪽 <b>II</b>를 끌어서 순서를 바꿀 수 있습니다.</p>`,
      target: () => retPage().querySelector('.sec[data-sec="ret-steps"]'), input: true, optional: true, needs: 'stepsSide' },
    { id: 'done', title: '설정이 완료되었습니다',
      body: `<p>매크로 복귀 설정이 끝났습니다.</p>
             <p>위쪽 <b>복귀 테스트</b>로 바로 확인할 수 있습니다. 정지는 F7 입니다.</p>` },
  ];
  TUTORIALS.push({ id: 'return', name: '매크로 복귀 설정', steps: RET_STEPS, skipAll: true, skipTimes: 3, menu: 'return',
    needed: () => needsRet(),
    skipMsg: '매크로 복귀 설정 튜토리얼을 건너뛰었습니다' });

  // 스나이핑 안정성 설정 — 꼭 진행 (건너뛰기 없음) · 맨 마지막 순서
  const snipePage = () => $('page-snipe');
  const snSec = sec => current === 'snipe' && sectionIs(sec, snipePage());
  const SNIPE_STEPS = [
    { title: '스나이핑 안정성 설정',
      body: `<p>일부 서버에는 <b>안티 스나이핑</b>(링크가 올라오자마자 들어오는 사람을 막는 장치)이 있습니다.</p>
             <p>이 설정으로 링크를 보고 <b>사람이 직접 누른 것처럼</b> 잠깐 기다렸다가 접속하게 할 수 있습니다.</p>
             <p class="dim">이 튜토리얼은 건너뛸 수 없습니다.</p>` },
    { id: 'card', title: '스나이핑 안정성 설정을 눌러주세요',
      body: `<p>스나이프 탭의 강조된 <b>스나이핑 안정성 설정</b> 버튼을 직접 눌러주세요.</p>`,
      target: () => document.querySelector('.snipe-open'), done: () => current === 'snipe' },
    { id: 'joinSide', ...secStep(snipePage, 'sn-join', '접속'), done: () => snSec('sn-join'), needs: 'card' },
    { title: '링크 감지 후 접속 딜레이',
      body: `<p>링크를 감지한 뒤 접속하기 전까지 <b>최소 ~ 최대 사이에서 매번 다르게</b> 기다립니다.</p>
             <p class="dim">기본 2 ~ 4초. 둘 다 0 이면 바로 접속합니다. 기다리는 중에 매크로를 중지하면 접속하지 않습니다.</p>`,
      target: () => $('snDelayRow'), input: true, needs: 'joinSide' },
    { title: '링크 열기 방식',
      body: `<p><b>켜짐</b>: 지금처럼 로블록스를 바로 실행합니다 (가장 빠름).</p>
             <p><b>꺼짐</b>: 기본 웹브라우저로 링크를 열어서, 사람이 링크를 누른 것과 같은 방법으로 접속합니다.</p>
             <p class="dim">꺼짐으로 쓸 때는 브라우저에서 처음 한 번 Roblox 열기를 누르면서 <b>항상 허용</b>에 체크해주세요. 그래야 다음부터 자동으로 열립니다.</p>`,
      target: () => $('snDirectRow'), input: true, needs: 'joinSide' },
    { title: '중복 · 연속 접속 막기',
      body: `<p><b>같은 서버 다시 안 들어가기</b>: 같은 서버 링크가 여러 채널에 올라와도 정한 시간(분) 동안 한 번만 들어갑니다.</p>
             <p><b>연속 접속 최소 간격</b>: 한 번 접속한 뒤 정한 시간(초) 동안은 다른 링크를 타지 않습니다.</p>
             <p class="dim">접속을 기다리는 중에 올라온 다른 링크도 건너뜁니다.</p>`,
      target: () => $('snRepeatRows'), input: true, needs: 'joinSide' },
    { id: 'done', title: '설정이 완료되었습니다',
      body: `<p>스나이핑 안정성 설정이 끝났습니다.</p>
             <p>언제든 스나이프 탭의 <b>스나이핑 안정성 설정</b>에서 바꿀 수 있습니다.</p>` },
  ];
  TUTORIALS.push({ id: 'snipe', name: '스나이핑 안정성 설정', steps: SNIPE_STEPS, menu: 'snipe',
    needed: () => !(config.tutorials_done || []).includes('snipe') });

  // 단계 참조(needs · skipTo)를 이름으로 쓸 수 있게 → 번호로 바꿈 (단계를 추가해도 번호가 안 꼬임)
  for (const t of TUTORIALS) {
    const idx = {};
    t.steps.forEach((s, n) => { if (s.id) idx[s.id] = n; });
    for (const s of t.steps) for (const k of ['needs', 'skipTo']) {
      if (typeof s[k] === 'string') {
        if (!(s[k] in idx)) console.error('튜토리얼 단계 참조 오류', t.id, s[k]);
        s[k] = idx[s[k]];
      }
    }
  }

  let tut = TUTORIALS[0];
  let STEPS = tut.steps;
  const doneList = () => config.tutorials_done || [];
  const skipped = new Set();     // 이번 실행에서 스킵한 튜토리얼 (남은 개수에서 제외)
  const remaining = () => TUTORIALS.filter(t => !doneList().includes(t.id) && !skipped.has(t.id)).length;
  function markDone(id) {
    if (!doneList().includes(id)) queueSave({ tutorials_done: [...doneList(), id] });
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
    $('tutStep').textContent = `${i + 1} / ${STEPS.length}`;
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
    const tabEl = focusEl?.closest('.menu-tab');
    // 탭이 미끄러져 들어오는 동안 강조 박스는 아래 follow 가 따라가며 제자리를 잡음
    if (tabEl && !tabEl.classList.contains('on')) setTab(tabEl.dataset.tab);
    place(!first);
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
  let lastRect = '';
  (function follow() {
    if (!el.hidden && focusEl && !busy) {
      const r = focusEl.getBoundingClientRect();
      const key = [r.left, r.top, r.width, r.height].map(v => Math.round(v)).join(',');
      if (key !== lastRect) {
        const first = !lastRect;
        lastRect = key;
        if (!first) place();
      }
    } else lastRect = '';
    requestAnimationFrame(follow);
  })();

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
  $('tutorialBtn').addEventListener('click', () => start('targets'));
  $('bioTutorialBtn').addEventListener('click', () => start('biome'));
  $('popTutorialBtn').addEventListener('click', () => start('popping'));
  $('retTutorialBtn').addEventListener('click', () => start('return'));
  $('snTutorialBtn').addEventListener('click', () => start('snipe'));

  // 튜토리얼은 항상 메인 화면에서 시작 (다른 화면이 열려 있으면 먼저 닫음)
  // chain: 자동으로 뜬 경우만 끝난 뒤 남은 튜토리얼로 이어감 ([튜토리얼 보기] 등 직접 연 경우는 그것만)
  async function start(id = 'targets', autoChain = false) {
    end();
    chain = autoChain;
    skipped.delete(id);
    tut = TUTORIALS.find(t => t.id === id) || TUTORIALS[0];
    STEPS = tut.steps;
    // 색 테마: 그 기능 버튼의 색으로
    el.style.setProperty('--tc', [...MENU, SNIPE_ENTRY].find(m => m.key === tut.menu)?.color || 'var(--accent)');
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
    maybeStart() {
      const first = TUTORIALS.find(t => t.needed());
      if (first) start(first.id, true);
    },
    isOpen: () => !el.hidden,
    // 화면이 다시 그려졌거나(위치 지정 후) 필수 값이 바뀌었을 때: 강조 대상·[다음] 상태 갱신
    refresh() {
      if (el.hidden || i < 0) return;
      const s = STEPS[i];
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

// ---------------------------------------------------------------- 시작
(async function init() {
  I18N.init();
  buildMenu();
  buildLang($('langBox'), false);
  buildLang($('tutLangBox'), true);
  setRun('idle');
  const s = await api('state');
  config = s.config;
  if (s.version) $('verText').textContent = 'V' + s.version;
  I18N.set(config.lang || I18N.detect());
  if (!config.lang) queueSave({ lang: I18N.lang });   // 처음 실행: 윈도우 언어로 정해서 저장
  I18N.onChange(() => { Tutorial.refresh(); requestAnimationFrame(() => setTab(curTab, false)); });   // 글자 길이가 바뀌면 간격 다시
  setTab(config.menu_tab || 'snipe', false);
  fillFields();
  bindFields();
  renderBiomes();
  renderLists();
  fillPlay();
  fillPop();
  fillRet();
  fillSnipe();
  fillSteps();
  fillBiome();
  fillMpop();
  renderMfish();
  renderMfAll();
  renderMpos('mpop'); renderMpos('mfish');
  fillAcrux();
  renderSummary();
  setStatus(s.status);
  armed = !!s.armed;
  setRun(armed ? 'running' : 'idle');
  renderSummary();
  poll();
  setTimeout(() => Tutorial.maybeStart(), 500);   // 감시 대상이 없으면 튜토리얼
})();
