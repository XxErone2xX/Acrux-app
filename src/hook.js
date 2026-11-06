// 디스코드 데스크톱 앱 안에 주입되는 스크립트.
// 내부 Flux 디스패처에 붙어서 새 메시지 이벤트를 파이썬으로 보낸다 (+ 화면 감시 폴백).
(() => {
  'use strict';
  // 디스코드 본 화면에서만 (안에 들어 있는 빈 프레임·about:blank 에서는 안 돌림)
  try { if (window.top !== window || !/(^|\.)discord\.com$/.test(location.hostname)) return; } catch { return; }
  const BINDING = '__lwEmit';
  const emit = (kind, data) => {
    try { window[BINDING](JSON.stringify({ kind, data })); } catch {}
  };

  // 이미 주입돼 있으면(파이썬 재접속 등) 상태만 다시 보고
  // 같은 디스코드 창에 예전 버전이 이미 들어가 있으면(디스코드를 안 껐을 때) 새 버전이 덮어씀
  const VERSION = 8;
  if (window.__lwInstalled >= VERSION) {
    // 이미 주입돼 있음: 아직 못 붙었으면 다시 찾기 시작, 아니면 상태만 다시 보고
    if (window.__lwRetry) window.__lwRetry();
    else if (window.__lwReport) window.__lwReport();
    return;
  }
  window.__lwInstalled = VERSION;

  let fluxOk = null;
  let ChannelStore = null, GuildStore = null, UserStore = null;
  window.__lwReport = () => emit('status', { flux: fluxOk });

  // 서버/채널 ID → 이름 (프로그램에서 호출)
  window.__lwLookup = ids => {
    if (!ChannelStore || !GuildStore) { try { scanBD() || scan(); } catch {} }
    const out = {};
    for (const id of ids || []) {
      try {
        const g = GuildStore && GuildStore.getGuild(id);
        if (g) { out[id] = { type: 'guild', name: g.name || '' }; continue; }
        const ch = ChannelStore && ChannelStore.getChannel(id);
        if (ch) {
          const pg = ch.guild_id && GuildStore && GuildStore.getGuild(ch.guild_id);
          const nm = ch.name || (ch.rawRecipients && ch.rawRecipients.map(r => r.global_name || r.username).join(', ')) || '';
          out[id] = { type: 'channel', name: nm, guildName: pg ? pg.name : (ch.guild_id ? '' : 'DM'), guildId: ch.guild_id || null };
        }
      } catch {}
    }
    return out;
  };

  // ---------- 웹팩 모듈 탐색 ----------
  // 디스코드 안에는 웹팩 실행기가 여러 개 있을 수 있음 (작은 것 + 실제 앱).
  // 모듈 꾸러미를 하나 밀어 넣으면 줄줄이 연결된 실행기마다 require 를 넘겨주므로 전부 모아서
  // 불러온 모듈이 가장 많은 것(= 실제 앱)부터 훑음
  const runtimes = [];
  const sizeOf = r => { try { return r && r.c ? Object.keys(r.c).length : 0; } catch { return 0; } };
  function getRuntimes() {
    const chunk = window.webpackChunkdiscord_app;
    if (!chunk || typeof chunk.push !== 'function') return runtimes;
    try {
      chunk.push([[Symbol('lw')], {}, r => { if (r && !runtimes.includes(r)) runtimes.push(r); }]);
      chunk.pop();
    } catch {}
    runtimes.sort((a, b) => sizeOf(b) - sizeOf(a));
    return runtimes;
  }

  const fn = (o, k) => { try { return typeof o[k] === 'function'; } catch { return false; } };
  const isDispatcher = c => c && typeof c === 'object' &&
    fn(c, 'subscribe') && fn(c, 'dispatch') && (c._subscriptions || c._actionHandlers);

  // 모듈 하나(exports)에서 디스패처 · 채널/서버 저장소 찾기
  let dispatcher = null;
  function check(exp) {
    if (!exp || (typeof exp !== 'object' && typeof exp !== 'function')) return;
    let cands = [exp];
    try { cands = cands.concat(Object.values(exp)); } catch {}
    try { if (exp.default && typeof exp.default === 'object') cands = cands.concat(Object.values(exp.default)); } catch {}
    for (const c of cands) {
      if (!c || typeof c !== 'object') continue;
      // 디스패처 비슷한 게 여러 개일 수 있음 → 구독이 가장 많은 것(= 실제 메시지가 흐르는 것)을 고름
      if (isDispatcher(c) && c !== dispatcher && subCount(c) > subCount(dispatcher)) dispatcher = c;
      if (!ChannelStore && fn(c, 'getChannel') && fn(c, 'getDMFromUserId')) ChannelStore = c;
      if (!GuildStore && fn(c, 'getGuild') && (fn(c, 'getGuildCount') || fn(c, 'getGuilds')) && !fn(c, 'getChannel')) GuildStore = c;
      if (!UserStore && fn(c, 'getCurrentUser') && fn(c, 'getUser')) UserStore = c;
    }
  }
  function subCount(d) {
    if (!d) return -1;
    let n = 0;
    try { n = Object.keys(d._subscriptions || {}).length; } catch {}
    try { const h = d._actionHandlers; if (h && h._orderedActionHandlers) n += Object.keys(h._orderedActionHandlers).length; } catch {}
    return n;
  }
  // 진짜 디스패처는 앱이 켜지면서 구독이 많이 붙어 있음 (새로 만든 사본·비슷한 다른 객체는 거의 0)
  const done = () => dispatcher && subCount(dispatcher) >= 3 && ChannelStore && GuildStore;

  // 모듈 코드에 이 글자가 들어 있으면 후보 (디스패처 · 저장소)
  const HINT = /_actionHandlers|_subscriptions|ChannelStore|GuildStore/;
  let hintIds = null;
  const hintCache = new WeakMap();

  function scan() {
    const rts = getRuntimes();
    if (!rts.length) return null;
    // 1) 실행기마다 이미 불러온 모듈을 훑음 (큰 것부터)
    for (const r of rts) {
      const cache = r.c;
      if (!cache) continue;
      for (const id in cache) {
        let exp;
        try { exp = cache[id] && cache[id].exports; } catch { continue; }
        check(exp);
      }
      if (done()) return dispatcher;
    }
    // 2) 그래도 없으면: 실행기마다 모듈 코드에서 후보만 골라 그 실행기의 require 로 가져옴
    //    (이미 불러온 모듈이면 그 실행기가 갖고 있는 진짜 것을 돌려줌) — 모듈이 가장 많은 실행기(= 실제 앱)부터
    const byFactories = rts.filter(r => r.m).sort((a, b) => Object.keys(b.m).length - Object.keys(a.m).length);
    for (const r of byFactories) {
      const ids = Object.keys(r.m);
      let h = hintCache.get(r);
      if (!h || h.count !== ids.length) {
        h = { count: ids.length, ids: ids.filter(id => {
          try { return HINT.test(Function.prototype.toString.call(r.m[id])); } catch { return false; }
        }) };
        hintCache.set(r, h);
      }
      hintIds = h.ids;
      for (const id of h.ids) {
        try { check(r.c && r.c[id] ? r.c[id].exports : r(id)); } catch {}
        if (done()) return dispatcher;
      }
    }
    return dispatcher;
  }

  // BetterDiscord가 깔려 있으면 BD의 웹팩 API를 먼저 사용 (가장 확실)
  function scanBD() {
    const W = window.BdApi && window.BdApi.Webpack;
    if (!W) return null;
    const get = f => {
      try { return W.getModule(f, { searchExports: true }) || W.getModule(f); } catch { return null; }
    };
    const store = n => {
      try { if (W.getStore) { const s = W.getStore(n); if (s) return s; } } catch {}
      return get(m => m && fn(m, 'getName') && m.getName() === n);
    };
    const d = get(isDispatcher);
    if (d) {
      ChannelStore = ChannelStore || store('ChannelStore');
      GuildStore = GuildStore || store('GuildStore');
    }
    return d;
  }

  function diag() {
    const chunk = window.webpackChunkdiscord_app;
    const rts = getRuntimes();
    emit('diag', {
      v: VERSION,
      url: location.href,
      chunk: !!chunk,
      chunkLen: chunk && chunk.length,
      pushPatched: !!(chunk && chunk.push !== Array.prototype.push),
      runtimes: rts.map(r => [sizeOf(r), r.m ? Object.keys(r.m).length : 0]),
      hints: hintIds ? hintIds.length : -1,
      found: { dispatcher: !!dispatcher, subs: subCount(dispatcher), channel: !!ChannelStore, guild: !!GuildStore },
      bd: !!(window.BdApi && window.BdApi.Webpack)
    });
  }

  // ---------- 메시지 정리 ----------
  function collect(m, parts = [], depth = 0) {
    if (!m || depth > 3) return parts;
    parts.push(m.content);
    for (const e of m.embeds || []) {
      parts.push(e.url, e.title, e.description,
        e.author && e.author.name, e.author && e.author.url, e.footer && e.footer.text);
      for (const f of e.fields || []) parts.push(f.name, f.value);
    }
    const walk = c => {
      if (!c) return;
      parts.push(c.url, c.label, c.content);
      (c.components || []).forEach(walk);
      walk(c.accessory);                       // 새 형식(Components V2) 섹션 옆 버튼
    };
    (m.components || []).forEach(walk);
    for (const s of m.message_snapshots || []) collect(s.message, parts, depth + 1);
    return parts;
  }

  function names(channelId, guildId) {
    const out = { channelName: '', guildName: '', parentId: null, thread: false };
    try {
      const ch = ChannelStore && ChannelStore.getChannel(channelId);
      if (ch) {
        out.channelName = ch.name || '';
        out.parentId = ch.parent_id || null;
        // 스레드 / 포럼 글 (타입 10, 11, 12) 은 감지 대상에서 제외
        out.thread = (typeof ch.isThread === 'function' && ch.isThread()) || [10, 11, 12].includes(ch.type);
        guildId = guildId || ch.guild_id;
      }
      const g = GuildStore && guildId && GuildStore.getGuild(guildId);
      if (g) out.guildName = g.name || '';
      out.dm = !guildId && (!ch || [1, 3].includes(ch.type));
    } catch {}
    out.guildId = guildId || null;
    return out;
  }

  // ---------- Prime Snipe: 메시지의 버튼 · 메뉴 (prime.py 의 fields 와 같은 모양) ----------
  const str = v => (v === null || v === undefined ? null : String(v));
  function primeFields(m) {
    const buttons = [], selects = [];
    const walk = (c, depth = 0) => {
      if (!c || typeof c !== 'object' || depth > 8) return;
      if (c.type === 2) {
        buttons.push({ label: String(c.label || ''), emoji: (c.emoji && c.emoji.name) || '', style: Number(c.style) || 0,
          customId: str(c.custom_id), url: c.url || null, disabled: !!c.disabled, n: buttons.length });
      } else if (c.type === 3) {
        selects.push({ customId: str(c.custom_id), placeholder: String(c.placeholder || ''), disabled: !!c.disabled, n: selects.length,
          options: (c.options || []).map(o => ({ label: String(o.label || ''), value: str(o.value), description: String(o.description || ''),
            emoji: (o.emoji && o.emoji.name) || '' })) });
      }
      (c.components || []).forEach(x => walk(x, depth + 1));
      walk(c.accessory, depth + 1);
    };
    (m.components || []).forEach(c => walk(c));
    const im = m.interaction_metadata || {}, it = m.interaction || {}, ref = m.message_reference || {};
    const flags = Number(m.flags) || 0, author = m.author || {};
    let me = null;
    try { me = UserStore && UserStore.getCurrentUser() ? String(UserStore.getCurrentUser().id) : null; } catch {}
    return {
      authorId: str(author.id), bot: !!author.bot, appId: str(m.application_id || m.webhook_id),
      flags, ephemeral: !!(flags & 64), loading: !!(flags & 128),
      interMsgId: str(im.interacted_message_id), interUser: str((im.user && im.user.id) || (it.user && it.user.id)),
      refId: str(ref.message_id), me, buttons, selects
    };
  }

  const ROBLOX_HINT = /roblox\.com|roblox:\/\//i;

  function onMessage(ev) {
    const m = ev && ev.message;
    if (!m || !m.id || ev.optimistic) return;
    const blob = collect(m).filter(Boolean).join('\n');
    const roblox = ROBLOX_HINT.test(blob);
    // Prime Snipe: 켜져 있으면 버튼 · 메뉴가 있는 메시지도 · 버튼을 누른 뒤 잠깐은 전부 (봇 응답)
    const watching = window.__lwPrimeWatch > Date.now();
    const pf = (roblox || watching || window.__lwPrimeOn) ? primeFields(m) : null;
    if (!roblox && !watching && !(window.__lwPrimeOn && pf && (pf.buttons.length || pf.selects.length))) return; // 파이썬으로 보낼 양 줄이기
    const channelId = m.channel_id || ev.channelId || null;
    const info = names(channelId, ev.guildId || m.guild_id);
    emit('msg', Object.assign({
      id: m.id, channelId, guildId: info.guildId, parentId: info.parentId, thread: info.thread,
      guildName: info.guildName, channelName: info.channelName,
      author: (m.author && (m.author.global_name || m.author.username)) || '',
      blob, via: 'flux', event: ev.type || 'MESSAGE_CREATE'
    }, pf || {}, { dm: !!info.dm }));
  }
  // 버튼을 누른 결과 (보냄 · 봇이 받음 · 실패 · 입력 창) — 누른 뒤 기다리는 동안만
  const IX = ['INTERACTION_CREATE', 'INTERACTION_SUCCESS', 'INTERACTION_FAILURE', 'INTERACTION_MODAL_CREATE'];
  function onIx(ev) {
    if (window.__lwPrimeWatch > Date.now()) emit('ix', { t: ev && ev.type, nonce: (ev && ev.nonce) || null });
  }

  // ---------- Flux 연결 (로딩 대기하며 재시도) ----------
  // 처음 2분은 1초마다, 그 뒤로도 못 붙었으면 '열린 채널만 감시' 로 알리고 5초마다 계속 시도
  let tries = 0, timer = null, subscribed = false;
  function attempt() {
    tries++;
    let d = null;
    try {
      d = scanBD();
      if (subCount(d) < 3) d = scan();
    } catch (e) { emit('diag', { error: String(e) }); }
    if (!d && dispatcher) d = dispatcher;
    // 구독이 거의 없는 건 가짜/다른 객체일 수 있어서 20초 동안은 더 찾아보고, 그래도 없으면 그거라도 씀
    if (d && subCount(d) < 3 && tries < 20) d = null;
    if (!d && (tries === 10 || tries === 120)) diag();
    if (d) {
      clearInterval(timer); timer = null;
      if (!subscribed) {
        d.subscribe('MESSAGE_CREATE', onMessage);
        d.subscribe('MESSAGE_UPDATE', onMessage);
        IX.forEach(t => { try { d.subscribe(t, onIx); } catch {} });
        subscribed = true;
      }
      fluxOk = true;
      window.__lwRetry = null;
      window.__lwReport();
    } else if (tries === 120) {
      fluxOk = false;
      window.__lwReport();
      clearInterval(timer);
      timer = setInterval(attempt, 5000);
    }
  }
  function startScan() {
    if (timer) clearInterval(timer);
    tries = 0;
    hintIds = null;
    timer = setInterval(attempt, 1000);
  }
  window.__lwRetry = () => { if (fluxOk) window.__lwReport(); else startScan(); };
  startScan();

  // ---------- Prime Snipe: 봇 버튼 누르기 (prime.py 가 Runtime.evaluate 로 부름) ----------
  // signal  · 누른 것처럼: 디스코드 앱이 버튼을 누를 때 보내는 '눌렀음' 신호를 앱 자신의 통신 기능으로 보냄
  //           (화면 이동 · 클릭 없음 · 로그인 정보는 앱 안에서만 쓰이고 밖으로 안 나감)
  // prepare · click · choose: 화면에서 누르기 (열려 있지 않으면 그 메시지로 이동) · restore: 원래 보던 화면으로
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const norm = s => String(s || '').replace(/\s+/g, ' ').trim().toLowerCase();
  const bare = s => norm(s).replace(/^:|:$/g, '');
  let backPath = null;
  // 불러온 모듈을 훑으면서 cb 가 true 인 것 (없으면 null)
  function eachExport(cb) {
    for (const r of getRuntimes()) {
      const cache = r.c;
      if (!cache) continue;
      for (const id in cache) {
        let exp;
        try { exp = cache[id] && cache[id].exports; } catch { continue; }
        if (!exp || (typeof exp !== 'object' && typeof exp !== 'function')) continue;
        let cands = [exp];
        try { cands = cands.concat(Object.values(exp)); } catch {}
        try { if (exp.default && typeof exp.default === 'object') cands = cands.concat(Object.values(exp.default)); } catch {}
        for (const c of cands) {
          try { if (c && (typeof c === 'object' || typeof c === 'function') && cb(c)) return c; } catch {}
        }
      }
    }
    return null;
  }
  // 디스코드 앱의 통신 기능 (get · post · put · patch · del) — 앱이 로그인 정보를 붙여서 보냄
  let Rest = null, Auth = null, routerFn, restAt = 0, authAt = 0;     // 못 찾았으면 30초 뒤에 다시 (모듈이 늦게 불러와질 수 있음)
  const isRest = c => typeof c === 'object' && ['get', 'post', 'put', 'patch', 'del'].every(k => fn(c, k));
  function findRest() {
    if (Rest) return Rest;
    if (Date.now() - restAt < 30000) return null;
    restAt = Date.now();
    let any = null;
    const best = eachExport(c => { if (!isRest(c)) return false; if (!any) any = c; return fn(c, 'getAPIBaseURL'); });
    Rest = best || any;
    return Rest;
  }
  // 이 앱의 통신 세션 ID (봇 응답 · '나만 보이는 메시지'가 이 앱으로 오게) — 로그인 정보는 안 읽음
  function sessionId() {
    if (!Auth && Date.now() - authAt > 30000) {
      authAt = Date.now();
      Auth = eachExport(c => typeof c === 'object' && fn(c, 'getSessionId') && fn(c, 'getName') && c.getName() === 'AuthenticationStore');
    }
    try { return Auth ? Auth.getSessionId() || null : null; } catch { return null; }
  }
  const nonce = () => String((BigInt(Date.now()) - 1420070400000n) << 22n);

  // 화면 이동: 디스코드 내부 이동 기능 (코드에 'Transitioning to' 가 있음) → 안 되면 주소만 바꾸고 알림
  function findRouter() {
    if (routerFn !== undefined) return routerFn;
    routerFn = null;
    for (const r of getRuntimes()) {
      if (!r.m) continue;
      for (const id of Object.keys(r.m)) {
        let src = '';
        try { src = Function.prototype.toString.call(r.m[id]); } catch { continue; }
        if (src.indexOf('Transitioning to') < 0) continue;
        let exp;
        try { exp = r.c && r.c[id] ? r.c[id].exports : r(id); } catch { continue; }
        const vals = [exp];
        try { vals.push(...Object.values(exp || {})); } catch {}
        for (const v of vals) {
          if (typeof v !== 'function') continue;
          let s2 = '';
          try { s2 = Function.prototype.toString.call(v); } catch {}
          if (s2.indexOf('Transitioning to') >= 0) { routerFn = v; return v; }
        }
      }
    }
    return null;
  }
  async function goTo(path) {
    const f = findRouter();
    if (f) { try { f(path); return 'router'; } catch {} }
    try {
      history.pushState(history.state, '', path);
      window.dispatchEvent(new PopStateEvent('popstate', { state: history.state }));
      return 'history';
    } catch {}
    return '';
  }

  const msgEl = (cid, mid) => document.getElementById(`chat-messages-${cid}-${mid}`);
  const textOf = el => norm(el.innerText || el.textContent || '');
  const emojiOf = el => { const im = el.querySelector('img[alt]'); return im ? bare(im.alt) : ''; };
  const area = li => li.querySelector('[id^="message-accessories-"]') || li;   // 메시지 아래 (도구 버튼 · 반응 빼고)
  function findButton(li, q) {
    const all = [...area(li).querySelectorAll('button, [role="button"]')]
      .filter(b => b.tagName !== 'A' && !b.closest('a[href], [class*="reaction"]'));
    const want = norm(q.label), wantE = bare(q.emoji);
    const same = all.filter(b => want ? textOf(b) === want : (!textOf(b) && wantE && emojiOf(b) === wantE));
    if (same.length) return same[Math.min(q.same || 0, same.length - 1)];
    return (want && all.find(b => textOf(b).includes(want))) || null;    // 글자 사이 이모지 등으로 조금 다르면
  }
  function findSelect(li, q) {
    const sels = [...area(li).querySelectorAll('[role="combobox"], [aria-haspopup="listbox"]')];
    if (!sels.length) return null;
    const ph = norm(q.placeholder);
    return (ph && sels.find(x => textOf(x).includes(ph))) || sels[Math.min(q.n || 0, sels.length - 1)];
  }
  const off = el => el.disabled || el.getAttribute('aria-disabled') === 'true';
  // 화면 클릭(대신): 마우스를 올리고 → 누르고 → 떼는 순서 그대로
  function fire(el) {
    const r = el.getBoundingClientRect();
    const x = r.left + r.width * (0.3 + Math.random() * 0.4), y = r.top + r.height * (0.3 + Math.random() * 0.4);
    const base = { cancelable: true, composed: true, view: window, clientX: x, clientY: y, screenX: x + screenX, screenY: y + screenY, button: 0 };
    const P = (t, b, bubbles = true) => el.dispatchEvent(new PointerEvent(t, Object.assign({}, base, { bubbles, pointerId: 1, pointerType: 'mouse', isPrimary: true, buttons: b })));
    const M = (t, b, bubbles = true) => el.dispatchEvent(new MouseEvent(t, Object.assign({}, base, { bubbles, buttons: b })));
    P('pointerover', 0); P('pointerenter', 0, false); M('mouseover', 0); M('mouseenter', 0, false);
    P('pointermove', 0); M('mousemove', 0);
    P('pointerdown', 1); M('mousedown', 1);
    try { el.focus({ preventScroll: true }); } catch {}
    P('pointerup', 0); M('mouseup', 0); M('click', 0);
  }

  window.__lwPrime = {
    async signal(q) {
      const rest = findRest();
      if (!rest) return { ok: false, reason: 'no-rest' };
      const sid = sessionId() || q.sessionId;
      if (!sid) return { ok: false, reason: 'no-session' };
      const data = q.kind === 'select'
        ? { component_type: 3, custom_id: q.customId, type: 3, values: [q.value] }
        : { component_type: 2, custom_id: q.customId };
      const body = { type: 3, nonce: nonce(), guild_id: q.guildId || undefined, channel_id: q.channelId,
        message_flags: q.flags || 0, message_id: q.messageId, application_id: q.appId, session_id: sid, data };
      try {
        const res = await Promise.race([rest.post({ url: '/interactions', body }), sleep(10000).then(() => 'timeout')]);
        if (res === 'timeout') return { ok: true, note: 'timeout' };
        const st = res && res.status;
        if (st && st >= 400) return { ok: false, reason: 'http ' + st };
        return { ok: true, status: st || 0 };
      } catch (e) {
        const st = e && (e.status || (e.res && e.res.status));
        const b = e && (e.body || (e.res && e.res.body));
        const m = b && (b.message || (b.errors && JSON.stringify(b.errors).slice(0, 100)));
        return { ok: false, reason: `http ${st || '?'}${m ? ' ' + m : ''}` };
      }
    },
    async prepare(q) {
      let li = msgEl(q.channelId, q.messageId), navigated = false;
      if (!li && q.navigate) {
        const here = location.pathname;
        if (await goTo(`/channels/${q.guildId || '@me'}/${q.channelId}/${q.messageId}`)) {
          navigated = true;
          if (!backPath) backPath = here;
          for (let i = 0; i < 60 && !(li = msgEl(q.channelId, q.messageId)); i++) await sleep(100);
        }
      }
      if (!li) return { ok: false, reason: 'no-message', navigated };
      const el = q.kind === 'select' ? findSelect(li, q) : findButton(li, q);
      if (!el) return { ok: false, reason: q.kind === 'select' ? 'no-select' : 'no-button', navigated };
      if (off(el)) return { ok: false, reason: 'disabled', navigated };
      try { el.scrollIntoView({ block: 'center', inline: 'nearest' }); } catch {}
      await sleep(150);
      const r = el.getBoundingClientRect();
      const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
      let hit = false;
      try { const top = document.elementFromPoint(cx, cy); hit = !!top && (top === el || el.contains(top)); } catch {}
      const visible = document.visibilityState === 'visible' && r.width > 4 && r.height > 4 && cx > 0 && cy > 0 && cx < innerWidth && cy < innerHeight;
      return { ok: true, navigated, hit, visible, rect: { x: r.left, y: r.top, w: r.width, h: r.height } };
    },
    async click(q) {
      const li = msgEl(q.channelId, q.messageId), el = li && findButton(li, q);
      if (!el) return { ok: false, reason: 'no-button' };
      if (off(el)) return { ok: false, reason: 'disabled' };
      fire(el);
      return { ok: true };
    },
    async choose(q) {
      const li = msgEl(q.channelId, q.messageId), sel = li && findSelect(li, q);
      if (!sel) return { ok: false, reason: 'no-select' };
      fire(sel);
      const want = norm(q.option);
      let opt = null;
      for (let i = 0; i < 30 && !opt; i++) {
        await sleep(100);
        opt = [...document.querySelectorAll('[role="option"]')].find(o => textOf(o).includes(want)) || null;
      }
      if (!opt) { this.escape(); return { ok: false, reason: 'no-option' }; }
      fire(opt);
      await sleep(200);
      return { ok: true };
    },
    async restore() {
      if (!backPath) return { ok: true };
      const p = backPath;
      backPath = null;
      if (location.pathname !== p) await goTo(p);
      return { ok: true, path: p };
    },
    escape() {
      const t = document.activeElement || document.body;
      for (const type of ['keydown', 'keyup']) {
        t.dispatchEvent(new KeyboardEvent(type, { key: 'Escape', code: 'Escape', keyCode: 27, which: 27, bubbles: true, cancelable: true }));
      }
      return { ok: true };
    },
    // 확인용 (어느 기능을 찾았는지)
    info() { return { rest: !!findRest(), session: !!sessionId(), router: !!findRouter(), path: location.pathname }; }
  };

  // ---------- DOM 폴백 (열려 있는 채널) ----------
  const SEL = 'li[id^="chat-messages-"]';
  const seen = new Set();
  function checkLi(li) {
    if (!li.id || seen.has(li.id)) return;
    seen.add(li.id);
    const mm = li.id.match(/^chat-messages-(\d+)-(\d+)$/);
    if (!mm) return;
    const hrefs = [...li.querySelectorAll('a[href]')].map(a => a.href);
    const blob = [li.textContent || '', ...hrefs].join('\n');
    if (!ROBLOX_HINT.test(blob)) return;
    const p = location.pathname.split('/');
    const info = names(mm[1], p[1] === 'channels' && p[2] !== '@me' ? p[2] : null);
    emit('msg', {
      id: mm[2], channelId: mm[1], guildId: info.guildId, parentId: info.parentId, thread: info.thread,
      guildName: info.guildName, channelName: info.channelName,
      author: '', blob, via: 'dom'
    });
  }
  function startObserver() {
    new MutationObserver(muts => {
      for (const mu of muts) for (const n of mu.addedNodes) {
        if (n.nodeType !== 1) continue;
        if (n.matches(SEL)) checkLi(n);
        else n.querySelectorAll(SEL).forEach(checkLi);
      }
    }).observe(document.body, { childList: true, subtree: true });
  }
  if (document.body) startObserver();
  else document.addEventListener('DOMContentLoaded', startObserver, { once: true });
})();
