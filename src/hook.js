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
  const VERSION = 7;
  if (window.__lwInstalled >= VERSION) {
    // 이미 주입돼 있음: 아직 못 붙었으면 다시 찾기 시작, 아니면 상태만 다시 보고
    if (window.__lwRetry) window.__lwRetry();
    else if (window.__lwReport) window.__lwReport();
    return;
  }
  window.__lwInstalled = VERSION;

  let fluxOk = null;
  let ChannelStore = null, GuildStore = null;
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
    } catch {}
    out.guildId = guildId || null;
    return out;
  }

  const ROBLOX_HINT = /roblox\.com|roblox:\/\//i;

  function onMessage(ev) {
    const m = ev && ev.message;
    if (!m || !m.id || ev.optimistic) return;
    const blob = collect(m).filter(Boolean).join('\n');
    if (!ROBLOX_HINT.test(blob)) return; // 파이썬으로 보낼 양 줄이기
    const channelId = m.channel_id || ev.channelId || null;
    const info = names(channelId, ev.guildId || m.guild_id);
    emit('msg', {
      id: m.id, channelId, guildId: info.guildId, parentId: info.parentId, thread: info.thread,
      guildName: info.guildName, channelName: info.channelName,
      author: (m.author && (m.author.global_name || m.author.username)) || '',
      blob, via: 'flux'
    });
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
