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
const MENU = [
  { key: 'biome', tab: 'biome', icon: 'globe', color: 'var(--cyan)', title: '바이옴 매크로 설정', desc: '바이옴 알림 · 디스코드 웹후크' },
  { key: 'discord', tab: 'snipe', icon: 'chat', color: 'var(--accent)', title: '디스코드 감지 설정', desc: '감시할 서버·채널, 필터, 감지 기록' },
  { key: 'popping', tab: 'snipe', icon: 'bolt', color: 'var(--orange)', title: '오토 팝핑 매크로 설정', desc: '게임 접속 후 Play 버튼 자동 클릭' },
  { key: 'return', tab: 'snipe', icon: 'undo', color: 'var(--green)', title: '매크로 복귀 설정', desc: '바이옴이 끝나면 내 서버로 복귀' },
  { key: 'mfeat', tab: 'macro', icon: 'bolt', color: 'var(--yellow)', title: '매크로 기능 설정', desc: '내 서버에서 돌릴 기능 켜기 · 끄기' },
  { key: 'mpos', tab: 'macro', icon: 'pin', color: 'var(--red)', title: '매크로 기준 위치 설정', desc: '통합 위치 · 기능별 버튼 위치 · 영역 지정' },
  { key: 'mstats', tab: 'macro', icon: 'chart', color: 'var(--cyan)', title: '통계 보기', desc: '이번 실행 · 올타임 기록' },
  { key: 'acrux', tab: 'acrux', icon: 'gear', color: '#7c6cf6', title: 'Acrux 설정', desc: 'OCR 감지 방식 · 언어 · 데이터 폴더',
    grad: 'linear-gradient(135deg, #8fa0ff, #6c7bff 50%, #8b5cf6)' },
  { key: 'updates', tab: 'acrux', icon: 'notes', color: 'var(--green)', title: '업데이트 로그', desc: '지금까지 바뀐 점 · 맨 위가 최신' },
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
  if (key === 'updates') loadChangelog();
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
const LOCAL_KEYS = ['play', 'biome', 'pop', 'ret', 'snipe', 'mpop', 'mfish', 'mitem', 'mmerch', 'mcraft', 'base', 'move'];
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
    config = (await api('set_config', { patch: p })).config;
    for (const [k, v] of Object.entries(keep)) if (v) config[k] = v;
    renderSummary();
  }
  catch { toast('설정 저장 실패'); }
}
// 기다리는 저장을 지금 바로 보냄 (서버가 방금 바꾼 설정을 써야 할 때)
async function flushSave() {
  if (!saveTimer) return;
  clearTimeout(saveTimer);
  await sendSave();
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
      toast(`오토 팝핑 설정 필요: ${r.missing.join(', ')}`);
      if (!Tutorial.isOpen()) Tutorial.start('popping');
      return;
    }
    setRun('running');
  }
  else { setRun('idle'); await api('stop'); }
  renderSummary();
});
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
document.querySelectorAll('[data-tab-go]').forEach(b => b.addEventListener('click', () => setTab(b.dataset.tabGo, true)));

// 켜진 매크로 탭 기능 수
const macroFeatures = () => [mpop().enabled, mfish().enabled, mitem().enabled, mmerch().enabled, mcraft().enabled].filter(Boolean).length;
let lastBio = null, lastMpop = null, lastMfish = null, lastMitem = null, lastMmerch = null, lastMcraft = null;
function syncMainTiles() {
  const bOn = !!bio().enabled;
  setTile('biome', bOn);
  const cur = lastBio && lastBio.current;
  $('mtBiome').textContent = bOn ? `작동 중 · ${cur || '바이옴 확인 중'}`
    : (bio().player ? '꺼짐' : '꺼짐 · 플레이어 이름 필요');
  $('mtSnipe').textContent = armed ? `감시 중 · ${$('count').textContent}개 감지` : '꺼짐';
  const mOn = !!config.macro_on, n = macroFeatures();
  const sniping = !!(lastBio && lastBio.muted);
  setTile('macro', mOn, mOn ? '중지 · F3' : '시작 · F3');
  tile('macro').classList.toggle('wait', mOn && sniping);
  $('mtMacro').textContent = !mOn ? (n ? `꺼짐 · 기능 ${n}개 켜짐` : '꺼짐')
    : lastMpop && lastMpop.running ? (lastMpop.msg || '포션 사용 중')
    : lastMitem && lastMitem.running ? (lastMitem.msg || '아이템 사용 중')
    : lastMmerch && lastMmerch.running && lastMmerch.job === 'buy' ? (lastMmerch.msg || '상인 구매 중')
    : lastMcraft && lastMcraft.running ? (lastMcraft.msg || '포션 제작 중')
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
    updateBiome(r.biome);
    updateMpop(r.mpop);
    updateMfish(r.mfish);
    updateMove(r.move);
    lastBio = r.biome; lastMpop = r.mpop; lastMfish = r.mfish; lastMitem = r.mitem; lastMmerch = r.mmerch; lastMcraft = r.mcraft;
    updateMitem(r.mitem);
    updateMcraft(r.mcraft);
    updateMmerch(r.mmerch);
    updateOnline(r.online);
    $('acOnline').closest('.row').hidden = !r.online_on;   // 사용자 수 서버가 아직 없으면 스위치도 숨김
    if (r.macro_on !== undefined && !!r.macro_on !== !!config.macro_on && Date.now() - macroClickAt > 2000)
      config.macro_on = r.macro_on;            // F3 · F7 로 켜고 끈 것
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

// 스나이프 오토 팝핑이 실제로 쓰는 위치 (통합 위치와 연동이면 통합 위치)
const popPos = () => {
  const p = pop(), b = base(), o = { ...p };
  const [first, other] = p.use_base ? [b, p] : [p, b];
  for (const k of POP_POS_KEYS) o[k] = first[k] || other[k];   // 고른 쪽에 비어 있으면 다른 쪽 값 (서버와 같게)
  return o;
};
function renderPopSet() {
  const box = $('popSetForm');
  const p = pop(), linked = !!p.use_base, e = popPos();
  const pick = (attr, k, label) => linked ? '' : `<button class="btn mini ghost" type="button" ${attr}${k ? `="${k}"` : ''}>${label}</button>`;
  box.innerHTML = `
    <label class="row"><span>매크로 통합 위치와 연동<small>켜면 통합 위치의 인벤토리 위치 · OCR 영역을 씀</small></span>
      <span class="switch"><input type="checkbox" id="popLink" ${linked ? 'checked' : ''}><i></i></span></label>` +
    (linked ? '' : tplRowHTML()) + POP_POS.map(([k, name, sub]) => `
    <div class="row"><span>${name} 위치<small>${sub ? sub + ' · ' : ''}${linked ? '통합 위치와 같음' : '직접 지정 필요'}</small></span>
      <span class="pos"><code class="${e[k] ? '' : 'unset'}" data-code="${k}">${fmtPos(e[k])}</code>
      ${pick('data-pick', k, '위치 지정')}</span></div>`).join('') + `
    <div class="row"><span>OCR 영역<small>검색 결과 아이템 이름·개수 (예: Warp Potion x23)${linked ? ' · 통합 위치와 같음' : ''}</small></span>
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
const base = () => (config.base ||= {});         // 통합 위치 (인벤토리 위치 · OCR 영역 · 알림 영역 …)
const saveMpop = () => queueSave({ mpop: JSON.parse(JSON.stringify(mpop())) });
const POP_POS_KEYS = ['inventory_pos', 'items_pos', 'search_pos', 'item_pos', 'amount_pos', 'use_pos', 'ocr_region'];
function renderMpop() {
  const m = mpop(), on = (m.biomes_on ||= {});
  const player = ((config.biome || {}).player || '').trim();
  const posMiss = POP_POS_KEYS.filter(k => !base()[k]).length;
  const label = { CYBERSPACE: 'Cyberspace', GLITCHED: 'Glitched', DREAMSPACE: 'Dreamspace' };
  $('mpopForm').innerHTML = `
    <label class="row"><span>켜기<small>레어 바이옴이 감지되면 아래 포션 목록대로 사용</small></span>
      <span class="switch"><input type="checkbox" id="mpopOn" ${m.enabled ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>플레이어 이름<small>바이옴 감지에 필요 · 바이옴 매크로 설정 → 기본 설정에서 입력</small></span>
      <span class="${player ? '' : 'warn'}">${player ? esc(player) : '입력 안 됨 — 바이옴 감지 안 됨'}</span></div>
    <div class="row"><span>버튼 위치 · OCR 영역<small>매크로 기준 위치 설정 → 통합 위치 에서 지정</small></span>
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
  ['target_pct', '목표 위치', '구간 왼쪽 변에서 몇 % 지점에서 누를지 · 0 = 왼쪽 변, 50 = 가운데 (%)', 0, 0, 5],
  ['lead_ms', '미리 누르기', '이만큼 미리 누름 · 늦게 눌리면 조금 늘림 (ms)', 0, 0, 10],
  ['click_gap_ms', '클릭 최소 간격', '릴링 중 클릭 사이 최소 간격 (ms)', 45, 10, 5],
  ['cast_retry', 'Fish 다시 누르기', '반응이 없을 때 다시 누르는 횟수 · 넘으면 인벤토리 가득', 3, 1, 1]];
function renderMfish() {
  const m = mfish();
  const posMiss = MFISH_REQ.filter(k => !m[k]).length;
  $('mfishForm').innerHTML = `
    <label class="row"><span>켜기<small>매크로가 켜져 있는 동안 계속 낚시</small></span>
      <span class="switch"><input type="checkbox" id="mfishOn" ${m.enabled ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>버튼 위치 · 릴링 바 영역<small>매크로 기준 위치 설정 → 자동 낚시 에서 지정</small></span>
      <span class="${posMiss ? 'warn' : ''}">${posMiss ? `${posMiss}개 지정 안 됨` : '지정됨'}</span></div>
    <div class="row"><span>이번 실행 기록<small>성공 · 쓰레기 · 실패 · 인벤토리 가득</small></span><b id="mfishStats">-</b></div>`;
  $('mfishOn').addEventListener('change', e => setFeature('mfish', e.target.checked));
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
  $('mfishDot').dataset.s = running ? 'flux' : '';
  $('mfishState').textContent = running ? (st.msg || '실행 중') : '대기';
  const s = st && st.stats, el = $('mfishStats');
  if (s && el) el.textContent = `${s.success} · ${s.junk} · ${s.fail} · ${s.full}`;
}
$('mfishStop').addEventListener('click', () => api('mfish_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 오토 아이템 사용
const mitem = () => (config.mitem ||= {});
const saveMitem = () => queueSave({ mitem: JSON.parse(JSON.stringify(mitem())) });
const MITEMS = [['strange', 'Strange Controller', 'strange_min', 10.5], ['randomizer', 'Biome Randomizer', 'randomizer_min', 18]];
const fmtLeft = s => s <= 0 ? '곧 사용' : s >= 60 ? `${Math.floor(s / 60)}분 ${Math.round(s % 60)}초 뒤` : `${Math.round(s)}초 뒤`;
function renderMitem() {
  const m = mitem();
  const posMiss = POP_POS_KEYS.filter(k => !base()[k]).length;
  $('mitemForm').innerHTML = `
    <label class="row"><span>켜기<small>매크로가 켜져 있는 동안 쿨타임마다 사용</small></span>
      <span class="switch"><input type="checkbox" id="mitemOn" ${m.enabled ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>인벤토리 위치 · OCR 영역<small>매크로 기준 위치 설정 → 통합 위치 에서 지정</small></span>
      <span class="${posMiss ? 'warn' : ''}">${posMiss ? `${posMiss}개 지정 안 됨` : '지정됨'}</span></div>` +
    MITEMS.map(([k, name, mk, def]) => `
    <div class="row"><span>${name}<small>켜면 이 간격(분)마다 1개 사용 · <b data-mitem-left="${k}">-</b></small></span>
      <span class="pos"><input type="number" min="1" step="0.5" data-mitem-min="${mk}" value="${m[mk] ?? def}" title="분">
      <label class="switch"><input type="checkbox" data-mitem-on="${k}" ${m[k] !== false ? 'checked' : ''}><i></i></label></span></div>`).join('') + `
    <label class="row"><span>다 쓰고 Inventory 닫기<small>Inventory 버튼을 한 번 더 눌러 닫음</small></span>
      <span class="switch"><input type="checkbox" id="mitemClose" ${m.close_inventory !== false ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>이번 실행 사용 횟수<small>Strange Controller · Biome Randomizer</small></span><b id="mitemUsed">-</b></div>`;
  $('mitemOn').addEventListener('change', e => setFeature('mitem', e.target.checked));
  $('mitemClose').addEventListener('change', e => { m.close_inventory = e.target.checked; saveMitem(); });
  $('mitemForm').querySelectorAll('[data-mitem-on]').forEach(i => i.addEventListener('change', () => { m[i.dataset.mitemOn] = i.checked; saveMitem(); }));
  $('mitemForm').querySelectorAll('[data-mitem-min]').forEach(i => i.addEventListener('input', () => {
    const n = parseFloat(i.value);
    if (Number.isFinite(n) && n >= 1) { m[i.dataset.mitemMin] = n; saveMitem(); }
  }));
  if (lastMitem) updateMitem(lastMitem);
}
function updateMitem(st) {
  const running = !!(st && st.running);
  $('mitemDot').dataset.s = running ? 'flux' : '';
  $('mitemState').textContent = running ? (st.msg || '사용 중') : '대기';
  $('mitemTest').disabled = running;
  const it = (st && st.items) || {};
  document.querySelectorAll('[data-mitem-left]').forEach(el => {
    const x = it[el.dataset.mitemLeft];
    el.textContent = !x ? '-' : !x.on ? '꺼짐' : !(config.macro_on && mitem().enabled) ? '매크로를 켜면 사용' : fmtLeft(x.left);
  });
  const u = $('mitemUsed');
  if (u && it.strange) u.textContent = `${it.strange.used} · ${it.randomizer.used}`;
}
$('mitemTest').addEventListener('click', async () => {
  const r = await api('mitem_test');
  toast(r.error || '오토 아이템 사용 테스트 시작 · 인벤토리 열기부터 · 정지: F7');
});
$('mitemStop').addEventListener('click', () => api('mitem_stop'));

// ---------------------------------------------------------------- 매크로 탭 · 포션 자동 제작
const mcraft = () => (config.mcraft ||= {});
const saveMcraft = () => queueSave({ mcraft: JSON.parse(JSON.stringify(mcraft())) });
const craftPlaceOk = () => (move().places || []).some(pl => pl.feat === 'mcraft' && pl.points.length && pl.points.every(pt => pt.pos && pt.time != null));
function renderMcraft() {
  const m = mcraft(), noticeOk = !!base().notice_region, placeOk = craftPlaceOk();
  $('mcraftForm').innerHTML = `
    <label class="row"><span>켜기<small>매크로가 켜져 있는 동안 알림을 확인</small></span>
      <span class="switch"><input type="checkbox" id="mcraftOn" ${m.enabled ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>알림 영역<small>매크로 기준 위치 설정 → 통합 위치 → 알림</small></span>
      <span class="${noticeOk ? '' : 'warn'}">${noticeOk ? '지정됨' : '지정 안 됨'}</span></div>
    <div class="row"><span>포션 제작 장소<small>매크로 기준 위치 설정 → 포션 자동 제작</small></span>
      <span class="${placeOk ? '' : 'warn'}">${placeOk ? '지정됨' : '지정 · 시간 재기 필요'}</span></div>
    <label class="row"><span>F 누른 뒤 대기<small>제작 창이 뜰 때까지 (초)</small></span>
      <input type="number" min="0.5" step="0.5" id="mcraftWait" value="${m.f_wait ?? 2.5}"></label>
    <div class="row"><span>제작 테스트<small>입력한 포션으로 지금 한 번 (예: Fortune Potion I)</small></span>
      <span class="pos"><input type="text" id="mcraftName" placeholder="포션 이름" maxlength="40">
      <button class="btn mini" type="button" id="mcraftTest">테스트</button></span></div>
    <div class="row"><span>이번 실행 제작 횟수</span><b id="mcraftCount">-</b></div>`;
  $('mcraftOn').addEventListener('change', e => setFeature('mcraft', e.target.checked));
  $('mcraftWait').addEventListener('input', e => {
    const n = parseFloat(e.target.value);
    if (Number.isFinite(n) && n >= 0.5) { m.f_wait = n; saveMcraft(); }
  });
  $('mcraftTest').addEventListener('click', async () => {
    const r = await api('mcraft_test', { name: $('mcraftName').value });
    toast(r.error || `제작 테스트 시작 — ${r.name} · 정지: F7`);
  });
  if (lastMcraft) updateMcraft(lastMcraft);
}
function updateMcraft(st) {
  const running = !!(st && st.running);
  $('mcraftDot').dataset.s = running ? 'flux' : '';
  $('mcraftState').textContent = running ? (st.msg || '제작 중') : '대기';
  const t = $('mcraftTest');
  if (t) t.disabled = running;
  const c = $('mcraftCount');
  if (c && st) c.textContent = st.crafted;
}
$('mcraftStop').addEventListener('click', () => api('mcraft_stop'));

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
  const need = ['chat_region', ...(m.auto_cal !== false ? [] : MMERCH_SHOP)];
  const miss = need.filter(k => !m[k]).length + (base().dialog_pos ? 0 : 1) + POP_POS_KEYS.filter(k => !base()[k]).length;
  $('mmerchForm').innerHTML = `
    <label class="row"><span>켜기<small>매크로가 켜져 있는 동안 채팅을 확인</small></span>
      <span class="switch"><input type="checkbox" id="mmerchOn" ${m.enabled ? 'checked' : ''}><i></i></span></label>
    <div class="row"><span>위치<small>매크로 기준 위치 설정 → 상인 자동 구매 · 통합 위치</small></span>
      <span class="${miss ? 'warn' : ''}">${miss ? `${miss}개 지정 안 됨` : '지정됨'}</span></div>` +
    MMERCH_SET.map(([k, name, unit, def, min, step]) => `
    <label class="row"><span>${name}<small>${unit}</small></span>
      <input type="number" min="${min}" step="${step}" data-mmerch-set="${k}" value="${m[k] ?? def}"></label>`).join('') + `
    <div class="row"><span>구매 테스트<small>상인이 와 있을 때 Merchant Teleporter 부터 한 번</small></span>
      <span class="pos"><button class="btn mini ghost" type="button" data-mmerch-buy="Mari">마리</button>
      <button class="btn mini ghost" type="button" data-mmerch-buy="Jester">제스터</button></span></div>
    <div class="row"><span>이번 실행 구매 횟수</span><b id="mmerchBought">-</b></div>`;
  // 상인마다 따로 칸
  $('mmerchItems').innerHTML = Object.entries(MERCH_ITEMS).map(([who, list]) => `
    <div class="sec-head mpop-tpl-head"><h3>${MERCH_NAME[who]}</h3></div>
    <div class="card form">` + list.map(n => {
    const k = `${who}_${n}`, on = buy[k] > 0;
    return `
      <div class="row"><span>${n}</span>
        <label class="switch"><input type="checkbox" data-mmerch-item="${k}" ${on ? 'checked' : ''}><i></i></label></div>`;
  }).join('') + `
    </div>`).join('');
  $('mmerchOn').addEventListener('change', e => setFeature('mmerch', e.target.checked));
  $('mmerchForm').querySelectorAll('[data-mmerch-set]').forEach(i => i.addEventListener('input', () => {
    const n = parseFloat(i.value);
    if (Number.isFinite(n) && n >= +i.min) { m[i.dataset.mmerchSet] = n; saveMmerch(); }
  }));
  $('mmerchForm').querySelectorAll('[data-mmerch-buy]').forEach(b => b.addEventListener('click', async () => {
    const r = await api('mmerch_buy_test', { name: b.dataset.mmerchBuy });
    toast(r.error || '구매 테스트 시작 · 정지: F7');
  }));
  $('mmerchItems').querySelectorAll('[data-mmerch-item]').forEach(i => i.addEventListener('change', () => {
    const k = i.dataset.mmerchItem;
    if (i.checked) buy[k] = 1; else delete buy[k];
    saveMmerch();
  }));
  if (lastMmerch) updateMmerch(lastMmerch);
}
function updateMmerch(st) {
  const running = !!(st && st.running);
  $('mmerchDot').dataset.s = running ? 'flux' : '';
  $('mmerchState').textContent = running ? (st.msg || '작동 중') : '대기';
  $('mmerchCheck').disabled = running;
  const b = $('mmerchBought');
  if (b && st) b.textContent = st.bought;
}
$('mmerchCheck').addEventListener('click', async () => {
  const r = await api('mmerch_check');
  toast(r.error || '채팅 확인 중 · 상인이 있으면 구매까지 이어감');
});
$('mmerchStop').addEventListener('click', () => api('mmerch_stop'));

// ---------------------------------------------------------------- 매크로 기능 설정 · 기능 켜기 · 끄기
// 준비 중인 기능은 자리만 (만들면 key 를 채움)
const MFEATS = [['mpop', '레어 바이옴 자동 팝핑', '내 서버에서 레어 바이옴이 뜨면 포션 사용'],
  ['mfish', '자동 낚시', '낚시 장소로 가서 낚시 · 가득 차면 판매'],
  ['mitem', '오토 아이템 사용', '아이템 자동 사용'],
  ['mmerch', '상인 자동 구매', '마리 · 제스터가 오면 고른 아이템 구매'], ['mcraft', '포션 자동 제작', 'Auto Crafted 가 뜨면 재료를 다시 채움'], [null, '오토 메모리 매치', '준비 중']];
const FEAT_NAME = { mpop: '레어 바이옴 자동 팝핑', mfish: '자동 낚시', mitem: '오토 아이템 사용', mmerch: '상인 자동 구매', mcraft: '포션 자동 제작' };
const featCfg = k => ({ mpop, mfish, mitem, mmerch, mcraft, base })[k]();
function setFeature(k, on, quiet) {
  const c = featCfg(k);
  c.enabled = !!on;
  queueSave({ [k]: JSON.parse(JSON.stringify(c)) });
  if (k === 'mfish' && !on) api('mfish_stop');
  if (k === 'mmerch' && !on) api('mmerch_stop');
  if (k === 'mcraft' && !on) api('mcraft_stop');
  // 다시 그리지 않고 같은 기능의 스위치만 맞춤 (다시 그리면 누른 스위치가 새로 생겨서 움직이는 애니메이션이 안 보임)
  document.querySelectorAll(`#${k}On, [data-feat="${k}"]`).forEach(i => { i.checked = !!on; });
  syncMainTiles();
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

// ---------------------------------------------------------------- 매크로 기준 위치 설정
// feat: 설정 묶음 (base = 여러 기능이 같이 쓰는 통합 위치 / mfish = 자동 낚시만) · points: [키, 이름, 설명] · region: [키, 이름, 설명]
// 저장은 서버(api)에서 · tpl: 화면 비율 위치 템플릿 · windows: 창 영역(드래그) → 안쪽 위치를 서버가 계산 · heads: 그 항목 앞의 소제목
const MPOS = {
  base: { box: 'mposBase', tpl: true, link: true,
          points: [...POP_POS.map(([k, n, sub]) => [k, n, sub || '']),
                   ['chat_pos', '채팅 버튼', '왼쪽 위의 채팅 버튼'],
                   ['collection_pos', '도감 버튼', '왼쪽 메뉴의 도감(Collection · 책 모양) 버튼'],
                   ['collection_close', '도감 Exit', '도감을 연 상태에서 Exit(닫기) 버튼'],
                   ['dialog_pos', '대화창', 'NPC 대화를 넘길 곳 (Click to skip.)']],
          regions: [['ocr_region', 'OCR 영역', '검색 결과 아이템 이름·개수 (예: Warp Potion x23)'],
                    ['notice_region', '알림 영역', '오른쪽 알림 카드 자리를 넉넉히 드래그']],
          // 칸: [id, 이름, 설명, 들어갈 항목, 맨 위에 연동 · 템플릿 줄을 넣을지, 따로 그리는 부분(MPOS_CUSTOM)]
          tabs: [['inv', '인벤토리', '레어 바이옴 자동 팝핑 등 인벤토리를 쓰는 기능',
                  [...POP_POS.map(([k]) => k), 'ocr_region'], true],
                 ['notice', '알림', '게임 알림을 보는 기능 (자동 낚시 인벤토리 가득 등)', ['notice_region']],
                 ['ui', '게임 버튼', '여러 기능이 같이 누르는 게임 화면 버튼', ['chat_pos', 'dialog_pos']],
                 ['move', '이동 · 기준 장소', '모든 이동의 출발점 (리셋 → 카메라 정렬 → 걷기 → 줌)',
                  ['collection_pos', 'collection_close'], false, 'movebase']] },
  mfish: { box: 'mposFish', tpl: true,
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
                  ['move', '이동', '매크로를 켜면 낚시 장소로, 가득 차면 판매 장소로 이동', [], false, 'places:mfish']] },
};
MPOS.mcraft = { box: 'mposCraft',
  points: [['search_pos', '검색창', '제작 창 오른쪽 위 Search'], ['shop_close_pos', '제작 창 X', "Stella's Workshop 오른쪽 위 X"],
           ['open_recipe_pos', 'Open Recipe 버튼', '왼쪽 아래 Open Recipe'], ['add_all_pos', 'Add Everything 버튼', 'Add Ingredients 창 왼쪽 아래'],
           ['craft_pos', 'Craft 버튼', 'Add Ingredients 창 오른쪽 아래'], ['add_close_pos', 'Add Ingredients 창 X', 'Add Ingredients 오른쪽 위 X']],
  regions: [['list_region', '검색 결과 목록', '검색창 아래 포션 목록 전체']],
  tabs: [['shop', '제작 창', '실행할 땐 글자로 먼저 찾고, 못 찾으면 이 위치를 씀',
          ['search_pos', 'list_region', 'shop_close_pos', 'open_recipe_pos', 'add_all_pos', 'craft_pos', 'add_close_pos'], false, '', 'craftauto'],
         ['revbase', '반대쪽 기준 장소', '낚시와 정반대 방향 · 리셋 → 카메라 정렬 → 내려다보기+줌 → S+D → S (끝에 A 같이)', [], false, 'movebaserev'],
         ['move', '이동', '1번 지점 = 퀘스트 보드 → (E → 대기 → Exit → D) → 2번 지점부터 Stella 앞까지', [], false, 'places:mcraft']] };
MPOS.mmerch = { box: 'mposMerch', tpl: true,
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
const MPOS_RATIOS = [['16:9', '16:9']];                // 다른 비율은 추정값이라 불안정해서 뺌
let mposRatio = '16:9';
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
    <label class="row"><span>스나이프 오토 팝핑과 연동<small>켜면 스나이프 오토 팝핑도 이 위치를 씀</small></span>
      <span class="switch"><input type="checkbox" data-mpos-link ${pop().use_base ? 'checked' : ''}><i></i></span></label>` : '';
  const tpl = d.tpl ? `
    <div class="row tpl-pick"><span>위치 템플릿<small>화면 비율에 맞는 기본 위치를 한 번에 채움</small></span>
      <span class="pos"><select data-mpos-ratio>${MPOS_RATIOS.map(([v, n]) => `<option value="${v}">${n}</option>`).join('')}</select>
      <button class="btn mini" type="button" data-mpos-tpl>적용</button></span></div>` : '';
  // 묶음마다 따로 칸(카드)으로 나눠서 보여 줌 · 제목 옆에 지정한 개수 / 전체 개수
  box.innerHTML = d.tabs.map(([id, name, sub, keys, top, custom, pre]) => {
    const n = keys.filter(k => c[k]).length;
    return `
    <div class="mpos-group" data-mpos-group="${id}">
      <div class="mpos-card-head"><b>${name}</b>${keys.length ? `<em class="${n === keys.length ? 'done' : ''}">${n}/${keys.length}</em>` : ''}</div>
      <p class="mpos-desc">${sub}</p>
      <div class="card form mpos-card">${pre ? `<div data-mpos-custom="${pre}"></div>` : ''}${top ? link + tpl : ''}${keys.map(k => items[k] || '').join('')}${custom ? `<div data-mpos-custom="${custom}"></div>` : ''}</div></div>`;
  }).join('');
  box.querySelectorAll('[data-mpos-custom]').forEach(renderMoveCustom);
  box.querySelector('[data-mpos-link]')?.addEventListener('change', e => setPopLink(e.target.checked));
  if (d.tpl) {
    const sel = box.querySelector('[data-mpos-ratio]');
    sel.value = mposRatio;
    sel.addEventListener('change', () => { mposRatio = sel.value; });
    box.querySelector('[data-mpos-tpl]').addEventListener('click', async e => {
      const b = e.currentTarget;
      const set = [...d.points, ...d.regions].some(([k]) => c[k]);
      if (set && !(b._armed > Date.now())) {             // 이미 지정한 게 있으면 한 번 더 눌러야 덮어씀
        b._armed = Date.now() + 3000; b.textContent = '한 번 더 누르면 덮어쓰기';
        setTimeout(() => { b.textContent = '적용'; b._armed = 0; }, 3000);
        return;
      }
      const ratio = sel.value;
      mposRatio = ratio;
      const r = await api('mpos_template', { feat, ratio });
      if (r.error) return toast(r.error);
      Object.assign(c, r[feat]); mposChanged(feat);
      toast(r.guess ? `${r.label} 템플릿 적용 · 추정값이라 [상태 확인] 으로 확인` : `${r.label} 템플릿 적용`);
    });
  }
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
// ---------------------------------------------------------------- 이동 (통합 위치 → 이동 탭)
// 기준 장소: 리셋 → Collection 열고 닫기(카메라 정렬) → 줌 · 장소: 지점 목록 [누를 곳, 걸린 시간]
const move = () => (config.move ||= { places: [] });
const saveMove = () => queueSave({ move: JSON.parse(JSON.stringify(move())) });
const MOVE_SET = [['reset_wait', '리셋 후 대기', 'Esc → R → Enter 로 리셋한 뒤 (초)', 3.5, 0.5, 0.1],
                  ['w_time', 'W 누르기', 'W 만 누르는 시간 (초)', 1, 0, 0.05],
                  ['wa_time', 'W + A 누르기', 'W 와 A 를 같이 누르는 시간 (초)', 7, 0, 0.5],
                  ['a_time', 'A 누르기', '그 다음 A 를 누르는 시간 (초)', 1, 0, 0.05],
                  ['w2_time', 'W 같이 누르기', 'A 를 누르는 마지막 이 시간 동안 W 도 같이 (초)', 0.25, 0, 0.05],
                  ['tilt_px', '화면 내려다보기', '우클릭을 누른 채 마우스를 아래로 끄는 거리 (px)', 800, 0, 50],
                  ['o_time', 'O 누르기', '최대 줌 (초)', 2.5, 0, 0.1],
                  ['margin', '도착 여유', '잰 시간에 더 기다릴 시간 (초)', 0.3, 0, 0.1]];
const REV_SET = [['rsd_time', 'S + D 누르기', 'S 와 D 를 같이 누르는 시간 (초)', 1, 0, 0.1],
                 ['rs_time', 'S 누르기', '그 다음 S 를 누르는 시간 (초)', 7, 0, 0.5],
                 ['ra_time', 'A 같이 누르기', 'S 를 누르는 마지막 이 시간 동안 A 도 같이 (초)', 1, 0, 0.1],
                 ['q_wait', '퀘스트 보드 E 뒤 대기', 'E 를 누르고 Exit 를 누르기까지 (초)', 1.5, 0, 0.1],
                 ['rd_time', 'D 누르기', 'Exit 를 누른 뒤 D 를 누르는 시간 (초)', 2, 0, 0.1]];
const revTime = m => 4.4 + (m.reset_wait ?? 3.5) + Math.max(m.o_time ?? 2.5, (m.tilt_px ?? 800) / 20 * 0.015)
  + (m.rsd_time ?? 1) + Math.max(m.rs_time ?? 7, m.ra_time ?? 1);
const questTime = m => 0.6 + (m.q_wait ?? 1.5) + (m.rd_time ?? 2);
let lastMove = null;
const fmtT = t => t == null ? '안 잼' : `${t}초`;
// 장소의 지점이 전부 지정 · 측정되면 제목 옆에 총 걸리는 시간 — Esc(리셋)부터 도착까지 전부
// 기준 장소 (서버 move.go_base 순서): 0.2 + Esc·R 1.0 + 리셋 대기 + 카메라 정렬 2.8 + W + W+A + 0.5 + 내려다보기·줌 + 0.4
const baseTime = m => 4.9 + (m.reset_wait ?? 3.5) + (m.w_time ?? 1) + (m.wa_time ?? 7) + Math.max(m.a_time ?? 1, m.w2_time ?? 0.25)
  + Math.max(m.o_time ?? 2.5, (m.tilt_px ?? 800) / 20 * 0.015);
const placeTotal = pl => {
  if (!pl.points.length || !pl.points.every(pt => pt.pos && pt.time != null)) return '';
  const rev = pl.feat === 'mcraft';
  const m = move(), t = (rev ? revTime(m) + questTime(m) : baseTime(m)) + pl.points.reduce((a, pt) => a + pt.time + (m.margin ?? 0.3), 0);
  return `<em class="move-total">총 ${Math.round(t * 10) / 10}초</em>`;
};
const MOVE_FIXED = ['mfish', 'mcraft'];                            // 자동 낚시: 낚시 장소 · 물고기 판매 장소 (서버 MOVE_TEMPLATES)
const moveCall = async (name, args, msg) => { const r = await api(name, args); toast(r.error || msg); return r; };
// data-mpos-custom: 'movebase' (통합 위치 → 기준 장소) / 'places:기능' (그 기능이 가는 장소)
function renderMoveCustom(el) {
  const [kind, feat] = el.dataset.mposCustom.split(':');
  if (kind === 'movebase') MPOS_CUSTOM.movebase(el);
  else if (kind === 'sell') MPOS_CUSTOM.sell(el);
  else if (kind === 'fishauto' || kind === 'sellauto' || kind === 'merchauto' || kind === 'craftauto' || kind === 'movebaserev') MPOS_CUSTOM[kind](el);
  else MPOS_CUSTOM.places(el, feat);
}
function rerenderMove() { document.querySelectorAll('[data-mpos-custom]').forEach(renderMoveCustom); if (config.mcraft) renderMcraft(); }
const SELL_SET = [['sell_delay', '클릭 사이 추가 대기', '렉이 있으면 늘림 (초)', 0, 0, 0.1],
                  ['sell_max', '판매 반복 최대', '클릭이 씹혀 끝없이 도는 것만 막음', 100, 1, 1]];
let sellcalBusy = false;
let autocalBusy = false;
let merchcalBusy = false;
let craftcalBusy = false;
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
      setTimeout(renderMmerch, 230);
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
        <div class="row move-point"><span>${j + 1}번 지점${pl.feat === 'mcraft' && j === 0 ? ' (퀘스트 보드)' : ''}<small>누를 곳 ${fmtPos(pt.pos)} · 걸린 시간 <b class="${pt.time == null ? 'warn' : ''}">${fmtT(pt.time)}</b></small></span>
          <span class="pos">
            <button class="btn mini ghost" type="button" data-move-pick="${i},${j},1" ${busy ? 'disabled' : ''}>기준 장소에서 지정</button>
            <button class="btn mini ghost" type="button" data-move-pick="${i},${j},0" ${busy ? 'disabled' : ''}>바로 지정</button>
            <button class="btn mini" type="button" data-move-test="${i},${j},1" ${busy || !pt.pos ? 'disabled' : ''}>시간 재기</button>
            ${pl.points.length > 1 ? `<button class="btn mini ghost" type="button" data-move-del-point="${i},${j}">삭제</button>` : ''}</span></div>`).join('') + `
        <div class="row"><span></span><span class="pos"><button class="btn mini ghost" type="button" data-move-add-point="${i}">지점 추가</button></span></div>
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
  document.querySelectorAll('.move-state').forEach(s => { s.textContent = now ? (st.msg || '이동 중') : '대기'; });
}
// 위치는 서버가 이미 저장함 → 화면만 다시 그림
function mposChanged(feat) {
  renderMpos(feat);
  if (feat === 'base') { renderMpop(); renderPopSet(); renderMitem(); renderMmerch(); renderMcraft(); }
  else if (feat === 'mmerch') renderMmerch();
  else if (feat === 'mcraft') renderMcraft();
  else renderMfish();
}
// 스나이프 탭 오토 팝핑 ↔ 통합 위치 연동 (양쪽 화면에서 같은 스위치)
function setPopLink(on) {
  pop().use_base = !!on; savePop();
  // 누른 스위치가 움직이는 걸 먼저 보여 주고 (바로 다시 그리면 스위치가 새로 생겨서 애니메이션이 안 보임) 끝나면 다시 그림
  document.querySelectorAll('#popLink, [data-mpos-link]').forEach(i => { i.checked = !!on; });
  clearTimeout(setPopLink.timer);
  setPopLink.timer = setTimeout(() => { renderMpos('base'); renderPopSet(); Tutorial.refresh(); }, 230);
  toast(on ? '스나이프 오토 팝핑이 통합 위치를 씀' : '스나이프 오토 팝핑은 따로 지정한 위치를 씀');
}
$('mposPopCopy').addEventListener('click', async () => {
  const r = await api('mpos_copy_pop');
  if (r.error) return toast(r.error);
  Object.assign(base(), r.base); mposChanged('base'); toast('스나이프 탭 오토 팝핑 위치를 가져옴');
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
  $('nowActiveN').textContent = n == null ? '--' : n;
  $('nowActive').classList.toggle('live', n != null);
}
$('acOnline').addEventListener('change', e => { config.online_share = e.target.checked; queueSave({ online_share: e.target.checked }); });
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
  const run = st && st.running;
  const playing = pst && pst.running && /복귀/.test(pst.msg || '') ;
  $('retDot').dataset.s = run ? 'flux' : '';
  $('retState').textContent = run ? (st.msg || '복귀 중') : '대기';
  $('retTest').disabled = !!run;
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
  const popMissing = () => !config.play?.pos || !config.play?.skip_pos || !popPos().ocr_region ||
    POP_POS.some(([k]) => !popPos()[k]);
  // 통합 위치와 연동이면 버튼 위치는 통합 위치에서 지정 (여기 튜토리얼로 안 띄움)
  const needsPop = () => popMissing() && !config.pop?.use_base && !(config.tutorials_done || []).includes('popping');

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
      target: pickRow(`[data-pick="${k}"]`), input: true, requires: () => !!popPos()[k], done: () => !!popPos()[k], needs: 'setSide',
      gif: { inventory_pos: 'inventory', items_pos: 'items', search_pos: 'search', item_pos: 'item', amount_pos: 'amount', use_pos: 'use' }[k] })),
    { title: 'OCR 영역을 지정해주세요',
      body: `<p><b>드래그로 지정</b>을 누른 뒤, 검색 결과 첫 칸의 <b>아이템 이름과 개수</b>(예: Warp Potion x23)가 들어가도록 드래그해주세요.</p>
             <p class="dim">필수 항목입니다. <b>OCR 테스트</b>로 제대로 읽히는지 확인할 수 있습니다.</p>`,
      target: pickRow('#popRegionPick'), input: true, requires: () => !!popPos().ocr_region, done: () => !!popPos().ocr_region, needs: 'setSide', gif: 'ocr' },
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
             <p><b>내 브섭 링크</b>는 꼭 입력해야 합니다. 돌아가면 켜 둔 매크로(자동 낚시 등)가 다시 시작됩니다.</p>`,
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
  // 스나이핑 안정성은 기본값이 있어서, 이미 하나라도 바꿨으면 설정한 것으로 봄 (튜토리얼을 안 거쳤어도)
  const SNIPE_DEF = { delay_min: 2, delay_max: 4, direct: true, same_link_min: 10, cooldown_sec: 5 };
  const snipeTouched = () => Object.entries(SNIPE_DEF).some(([k, v]) => config.snipe?.[k] !== undefined && config.snipe[k] !== v);
  TUTORIALS.push({ id: 'snipe', name: '스나이핑 안정성 설정', steps: SNIPE_STEPS, menu: 'snipe',
    needed: () => !(config.tutorials_done || []).includes('snipe') && !snipeTouched() });

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
  // 남은 튜토리얼: 지금 하는 것 + 아직 필요한 것만 (설정이 이미 돼 있어서 안 뜰 튜토리얼은 세지 않음)
  const remaining = () => TUTORIALS.filter(t => !doneList().includes(t.id) && !skipped.has(t.id)
    && (t === tut || t.needed())).length;
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
  fillBiome();
  fillMpop();
  renderMfish();
  renderMitem(); renderMmerch(); renderMcraft();
  renderMfAll();
  renderMpos('base'); renderMpos('mfish'); renderMpos('mmerch'); renderMpos('mcraft');
  fillAcrux();
  renderSummary();
  setStatus(s.status);
  armed = !!s.armed;
  setRun(armed ? 'running' : 'idle');
  renderSummary();
  poll();
  setTimeout(() => Tutorial.maybeStart(), 500);   // 감시 대상이 없으면 튜토리얼
})();
