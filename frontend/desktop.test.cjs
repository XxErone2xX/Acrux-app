const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const ts = require('typescript');

async function flush() { for (let i = 0; i < 24; i++) await Promise.resolve(); }

function desktop(saved, options = {}) {
  const preferencesKey = 'acrux.desktop.preferences';
  const storage = new Map([[preferencesKey, JSON.stringify(saved)]]);
  const errors = [], calls = [];
  const elements = [];
  let time = 0, timerId = 0;
  const timers = new Map();
  const schedule = (fn, delay) => { const id = ++timerId; timers.set(id, { fn, at: time + delay }); return id; };
  class Element {
    constructor(tag) { this.tag = tag; this.style = {}; this.children = []; this.listeners = {}; elements.push(this); }
    setAttribute(name, value) { this[name] = value; }
    append(...nodes) { this.children.push(...nodes); }
    addEventListener(name, callback) { (this.listeners[name] ??= []).push(callback); }
    removeEventListener(name, callback) { this.listeners[name] = (this.listeners[name] ?? []).filter(fn => fn !== callback); }
    emit(name, event = {}) { for (const callback of this.listeners[name] ?? []) callback({ preventDefault() {}, stopPropagation() {}, ...event }); }
    remove() { this.removed = true; }
    close() { this.open = false; this.emit('close'); }
    showModal() { this.open = true; options.onDialog?.(this); }
    focus() {}
    getBoundingClientRect() { return { left: 20, top: 30, width: 200, height: 100 }; }
    getContext() { return { putImageData: image => { this.image = image; } }; }
  }
  const window = new Element('window');
  window.dispatchEvent = event => errors.push(event.detail);
  const stats = { success: 0, junk: 0, fail: 0, full: 0 };
  const status = {
    phase: 'idle', positions: null,
    tuning: { biteSeconds: 60, leadMs: 0, targetPercent: 0, clickGapMs: 45, castRetries: 3 }, stats, totals: stats,
    message: '대기 중', gameAvailable: false, version: '0.1.0',
    measuredSeconds: null, measuringElapsed: null,
    generation: 0,
    session: { supported: true, state: "ready", message: "앱 전용 게임 준비됨", width: 2, height: 1, ...options.session },
    dua: { supported: true, background: options.background !== false, hotkeys: false, running: false, route: null, reason: '대기 중', headlessTicks: 0, screenshotFrames: 0, transitions: 0 },
  };
  const biome = { settings: { enabled: false, player: '', psLink: '', webhooks: [], mentions: {}, roles: {} } };
  const discord = { linked: false, name: '', code: null, error: '', ...options.discord };
  const modules = {
    './desktop-defaults.json': JSON.parse(fs.readFileSync(path.join(__dirname, 'desktop-defaults.json'), 'utf8')),
    './engine': {
      status: async () => {
        const current = structuredClone(status);
        if (options.statusWait) await options.statusWait();
        return current;
      },
      control: async action => {
        calls.push(['control', action]);
        if (action === 'stop') { status.dua.running = false; status.phase = 'idle'; status.generation++; }
        return structuredClone(status);
      },
      saveTuning: async tuning => { calls.push(['save_tuning', structuredClone(tuning)]); status.tuning = tuning; },
    },
    './biome': { biomeStatus: async () => structuredClone(biome), saveBiome: async settings => ({ settings }) },
    '@tauri-apps/api/core': { invoke: async (command, payload) => {
      calls.push([command, structuredClone(payload)]);
      if (command === 'online_status') return { share: true, online: 1 };
      if (command === 'save_online') return;
      if (command === 'discord_state') return structuredClone(discord);
      if (command === 'discord_login') {
        discord.code = options.discordError ? null : 'ABCD1234';
        discord.error = options.discordError || '';
        return structuredClone(discord);
      }
      if (command === 'discord_cancel' || command === 'discord_logout') {
        Object.assign(discord, { code: null, error: '' });
        if (command === 'discord_logout') Object.assign(discord, { linked: false, name: '' });
        return structuredClone(discord);
      }
      if (command === 'desktop_session_start') {
        if (!status.session.supported) throw new Error('지원하지 않는 환경입니다.');
        status.session.state = 'ready'; status.dua.background = true;
        return structuredClone(status.session);
      }
      if (command === 'desktop_session_stop') {
        status.session.state = 'stopped'; status.dua.background = false;
        status.dua.running = false; status.phase = 'idle'; status.generation++;
        return structuredClone(status.session);
      }
      if (command === 'desktop_manual') {
        if (payload.event.kind !== 'release') {
          if (payload.generation !== status.generation) throw new Error('이전 입력이 취소됐습니다.');
          if (status.dua.running || status.phase !== 'idle') throw new Error('매크로를 먼저 중지하세요.');
        }
        if (options.manualWait) await options.manualWait(payload);
        return;
      }
      if (command === 'desktop_snapshot') {
        if (options.snapshotWait) await options.snapshotWait();
        if (!options.snapshot) throw new Error('No snapshot fixture configured');
        return options.snapshot;
      }
      if (command === 'desktop_navigation') { if (options.navigationWait) await options.navigationWait(); return; }
      if (command === 'desktop_start') {
        if (options.startWait) await options.startWait();
        if (payload.generation !== status.generation) throw new Error('이전 실행 요청이 취소됐습니다.');
        if (options.startError) throw new Error(options.startError);
        status.generation++;
        Object.assign(status.dua, { running: true, route: 'headless', reason: '상태 확인 중' });
        return structuredClone(status);
      }
      throw new Error(`Unexpected command ${command}`);
    } },
  };
  const source = fs.readFileSync(path.join(__dirname, 'desktop.ts'), 'utf8');
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, esModuleInterop: true },
  }).outputText;
  vm.runInNewContext(compiled, {
    exports: {}, require: name => {
      assert.ok(name in modules, `Unexpected module ${name}`);
      return modules[name];
    },
    window, document: { createElement: tag => new Element(tag), body: new Element('body') },
    location: { href: 'http://localhost/desktop/index.html' },
    localStorage: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) },
    structuredClone, URL, console, ArrayBuffer, DataView, Uint8ClampedArray,
    setTimeout: schedule, clearTimeout: id => timers.delete(id),
    ImageData: class { constructor(data, width, height) { this.data = data; this.width = width; this.height = height; } },
    CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
  });
  return {
    api: window.acruxDesktopApi,
    stored: () => JSON.parse(storage.get(preferencesKey)),
    status, calls, errors, elements, window, timers, discord,
    invoke: modules['@tauri-apps/api/core'].invoke,
    async tick(milliseconds) {
      time += milliseconds;
      for (const [id, timer] of [...timers]) {
        if (timer.at <= time) { timers.delete(id); timer.fn(); }
      }
      await flush();
    },
  };
}

test('old desktop preferences retain shared positions and remove retired settings', async () => {
  const app = desktop({
    menu_tab: 'snipe', guild_ids: ['123'], snipe: { enabled: true }, prime: { key: 'retired' },
    pop: { inventory_pos: [0.1, 0.2], search_pos: [0.3, 0.4], match_threshold: 50,
      templates: { GLITCHED: {} }, favs: ['GLITCHED'], use_base: true, delays: { after_join: 99, search: 0.2 } },
    base: { search_pos: [0.5, 0.6] },
    ret: { ps_link: 'own', start_wait: 9, after_macro: false, steps: [{ type: 'text' }] },
    mods: { sniping: { enabled: true }, example: { enabled: false } },
    mod_settings: { sniping: { token: 'retired' }, example: { volume: 1 } },
  });
  const config = app.stored();
  for (const key of ['snipe', 'prime', 'guild_ids']) assert.ok(!(key in config));
  assert.equal(config.menu_tab, 'macro');
  assert.deepEqual(config.base.inventory_pos, [0.1, 0.2]);
  assert.deepEqual(config.base.search_pos, [0.5, 0.6]);
  assert.deepEqual(config.pop, { match_threshold: 50, delays: { search: 0.2 } });
  assert.deepEqual(config.ret, { ps_link: 'own', start_wait: 9 });
  assert.deepEqual(config.mods, { example: { enabled: false } });
  assert.deepEqual(config.mod_settings, { example: { volume: 1 } });
  assert.ok((await app.api('state', {})).config.mfish);
});

test('main start requires the private session and starts DUA with the enabled feature list', async () => {
  const app = desktop({ mfish: { enabled: true } });
  const result = await app.api('set_config', { patch: { macro_on: true } });
  assert.equal(result.config.macro_on, true);
  assert.deepEqual(app.calls.map(([name]) => name), ['desktop_start']);
  assert.deepEqual(app.calls[0][1], { features: ['mfish'], generation: 0 });
  assert.equal(app.calls.some(([name, action]) => name === 'control' && action === 'start'), false);
  assert.equal(app.stored().macro_on, false, 'relaunch must not resume automation');
});

test('DUA idle thinking remains running and both routes and counters reach poll', async () => {
  const app = desktop({ mfish: { enabled: true } });
  await app.api('set_config', { patch: { macro_on: true } });
  assert.equal(app.status.phase, 'idle');
  let result = await app.api('poll', {});
  assert.equal(result.macro_on, true);
  assert.deepEqual(result.dua, app.status.dua);
  assert.deepEqual([...result.status], ['flux', '상태 확인 중']);
  Object.assign(app.status.dua, { route: 'screenshot', reason: '낚시 화면 확인', headlessTicks: 4, screenshotFrames: 2, transitions: 3 });
  result = await app.api('poll', {});
  assert.equal(result.macro_on, true);
  assert.deepEqual(result.dua, app.status.dua);
  assert.match(result.logs[0].msg, /screenshot.*낚시 화면 확인.*대기 중/);
  app.status.message = '릴링 중';
  result = await app.api('poll', {});
  assert.match(result.logs[0].msg, /릴링 중/);
  app.status.dua.running = false;
  result = await app.api('poll', {});
  assert.equal(result.macro_on, false);
});

test('stop reaches DUA while the fishing engine is idle or calibrating', async () => {
  for (const phase of ['idle', 'calibrating']) {
    const app = desktop({ mfish: { enabled: true } });
    await app.api('set_config', { patch: { macro_on: true } });
    app.status.phase = phase;
    await app.api('mfish_stop', {});
    assert.equal(app.status.dua.running, false);
    assert.ok(app.calls.some(([name, action]) => name === 'control' && action === 'stop'));
    assert.equal((await app.api('poll', {})).macro_on, false);
  }
});

test('start errors turn the macro off and report the backend reason', async () => {
  const app = desktop({ mfish: { enabled: true } }, { startError: 'Roblox 화면을 찾을 수 없습니다.' });
  const result = await app.api('set_config', { patch: { macro_on: true } });
  assert.equal(result.config.macro_on, false);
  assert.match(app.errors[0], /Roblox 화면을 찾을 수 없습니다/);
});

test('stop cancels a start waiting at the backend', async () => {
  for (const stop of ['mfish_stop', 'set_config']) {
    for (const hook of ['startWait']) {
      let entered, resume;
      const waiting = new Promise(resolve => { entered = resolve; });
      const gate = new Promise(resolve => { resume = resolve; });
      const app = desktop({ mfish: { enabled: true } }, { [hook]: async () => { entered(); await gate; } });
      const pending = app.api('set_config', { patch: { macro_on: true } });
      await waiting;
      await app.api(stop, stop === 'set_config' ? { patch: { macro_on: false } } : {});
      resume();
      const result = await pending;
      assert.equal(result.config.macro_on, false);
      assert.equal(app.status.dua.running, false);
      assert.equal(app.errors.length, 0, 'canceled requests must not report stale errors');
    }
  }
});

test('start sends every enabled feature so unsupported requests cannot run silently', async () => {
  const app = desktop({ mfish: { enabled: true }, mitem: { enabled: true } }, { startError: '지원하지 않는 기능: mitem' });
  const result = await app.api('set_config', { patch: { macro_on: true } });
  assert.deepEqual(app.calls.find(([name]) => name === 'desktop_start')[1], { features: ['mfish', 'mitem'], generation: 0 });
  assert.equal(result.config.macro_on, false);
  assert.match(app.errors[0], /mitem/);
});

test('running edits do not restart DUA and changing enabled features stops it', async () => {
  const app = desktop({ mfish: { enabled: true } });
  await app.api('set_config', { patch: { macro_on: true } });
  await app.api('set_config', { patch: { mfish: { enabled: true, sell_max: 80 } } });
  assert.equal(app.calls.filter(([name]) => name === 'desktop_start').length, 1);
  assert.equal(app.calls.filter(([name]) => name === 'desktop_navigation').length, 0);
  assert.equal(app.status.dua.running, true);
  const result = await app.api('set_config', { patch: { mfish: { enabled: false } } });
  assert.equal(result.config.macro_on, false);
  assert.equal(app.status.dua.running, false);
  assert.match(app.errors.at(-1), /기능이 바뀌어/);
});

test('saving fishing preferences does not cancel a separate manual calibration', async () => {
  const app = desktop({ mfish: { enabled: false } });
  app.status.phase = 'calibrating';
  await app.api('set_config', { patch: { mfish: { enabled: true } } });
  assert.equal(app.calls.length, 0);
  assert.equal(app.status.phase, 'calibrating');
  await app.api('set_config', { patch: { macro_on: false } });
  assert.deepEqual(app.calls, [['control', 'stop']]);
});

function snapshot(width, height, pixels = []) {
  const buffer = new ArrayBuffer(8 + pixels.length);
  const header = new DataView(buffer);
  header.setUint32(0, width, true);
  header.setUint32(4, height, true);
  new Uint8Array(buffer, 8).set(pixels);
  return buffer;
}

test('private game picks a normalized position from the actual snapshot pixels inside the app', async () => {
  let shown;
  const opened = new Promise(resolve => { shown = resolve; });
  const pixels = [255, 0, 0, 255, 0, 255, 0, 255];
  const app = desktop({}, { snapshot: snapshot(2, 1, pixels), onDialog: shown });
  const pending = app.api('mpos_point', { feat: 'base', key: 'chat_pos' });
  const dialog = await opened;
  assert.equal(dialog.open, true);
  const canvas = app.elements.find(el => el.tag === 'canvas');
  assert.deepEqual([...canvas.image.data], pixels);
  assert.equal(canvas.image.width, 2);
  canvas.emit('click', { clientX: 70, clientY: 105 });
  const result = await pending;
  assert.deepEqual([...result.pos], [0.25, 0.75]);
  assert.deepEqual(app.stored().base.chat_pos, [0.25, 0.75]);
  assert.equal(dialog.removed, true);
  assert.equal(app.calls.some(([name]) => name === 'desktop_pick'), false);
});

test('snapshot picker rejects invalid dimensions and truncated or extra pixel data before creating a dialog', async () => {
  for (const data of [new ArrayBuffer(4), snapshot(0, 1), snapshot(16384, 16384), snapshot(2, 1, [1, 2, 3, 4]), snapshot(1, 1, [1, 2, 3, 4, 5])]) {
    const app = desktop({}, { snapshot: data });
    const result = await app.api('mpos_point', { feat: 'base', key: 'chat_pos' });
    assert.match(result.error, /Sober 화면/);
    assert.equal(app.elements.some(el => el.tag === 'dialog'), false);
    assert.equal(app.stored().base.chat_pos, null);
  }
});

test('snapshot picker cancels with Escape, its cancel button or pick_cancel without saving a position', async () => {
  for (const cancel of ['escape', 'button', 'api']) {
    let shown;
    const opened = new Promise(resolve => { shown = resolve; });
    const app = desktop({}, { snapshot: snapshot(1, 1, [1, 2, 3, 255]), onDialog: shown });
    const pending = app.api('mpos_point', { feat: 'base', key: 'chat_pos' });
    const dialog = await opened;
    if (cancel === 'escape') dialog.emit('keydown', { key: 'Escape' });
    if (cancel === 'button') app.elements.find(el => el.tag === 'button').emit('click');
    if (cancel === 'api') await app.api('pick_cancel', {});
    const result = await pending;
    assert.match(result.error, /취소/);
    assert.equal(dialog.removed, true);
    assert.equal(app.stored().base.chat_pos, null);
  }
});

test('canceling a pending snapshot cannot reopen a stale picker', async () => {
  let entered, resume;
  const started = new Promise(resolve => { entered = resolve; });
  const gate = new Promise(resolve => { resume = resolve; });
  const app = desktop({}, { snapshot: snapshot(1, 1, [1, 2, 3, 255]), snapshotWait: async () => { entered(); await gate; } });
  const pending = app.api('mpos_point', { feat: 'base', key: 'chat_pos' });
  await started;
  await app.api('pick_cancel', {});
  resume();
  assert.match((await pending).error, /취소/);
  assert.equal(app.elements.some(el => el.tag === 'dialog'), false);
});

test('shared host fallback is refused for both main start and position picking', async () => {
  const app = desktop({ mfish: { enabled: true } }, { background: false, session: { state: 'stopped' } });
  const result = await app.api('mpos_point', { feat: 'base', key: 'chat_pos' });
  assert.match(result.error, /앱 전용 게임/);
  const started = await app.api('set_config', { patch: { macro_on: true } });
  assert.equal(started.config.macro_on, false);
  assert.match(app.errors[0], /앱 전용 게임/);
  assert.equal(app.calls.some(([name]) => ['desktop_pick', 'desktop_start', 'desktop_navigation'].includes(name)), false);
});

test('session start and stop use only the private session commands and publish its status', async () => {
  const app = desktop({}, { background: false, session: { state: 'stopped' } });
  assert.equal((await app.api('state', {})).session.state, 'stopped');
  assert.equal((await app.api('session_start', {})).session.state, 'ready');
  assert.equal((await app.api('poll', {})).session.state, 'ready');
  assert.equal((await app.api('session_stop', {})).session.state, 'stopped');
  assert.equal(app.status.dua.background, false);
  assert.deepEqual(app.calls.filter(([name]) => name.startsWith('desktop_session')).map(([name]) => name), ['desktop_session_start', 'desktop_session_stop']);
  const unsupported = desktop({}, { background: false, session: { supported: false, state: 'unsupported' } });
  assert.match((await unsupported.api('session_start', {})).error, /지원하지 않는/);
  assert.match((await unsupported.api('game_view', {})).error, /앱 전용 게임/);
});

test('game view renders actual pixels, has one capture in flight, and stops after close', async () => {
  let entered, resume;
  const waiting = new Promise(resolve => { entered = resolve; });
  const gate = new Promise(resolve => { resume = resolve; });
  let captures = 0;
  const pixels = [255, 0, 0, 255, 0, 255, 0, 255];
  const app = desktop({}, { snapshot: snapshot(2, 1, pixels), snapshotWait: async () => { if (++captures === 1) { entered(); await gate; } } });
  await app.api('game_view', {});
  await waiting;
  await app.tick(2000);
  assert.equal(captures, 1, 'the next frame waits for the in-flight frame');
  resume(); await flush();
  const canvas = app.elements.find(el => el.tag === 'canvas');
  assert.deepEqual([...canvas.image.data], pixels);
  await app.tick(499);
  assert.equal(captures, 1);
  await app.tick(1);
  assert.equal(captures, 2);
  await app.api('game_view_close', {}); await flush();
  const dialog = app.elements.find(el => el.tag === 'dialog');
  assert.equal(dialog.removed, true);
  assert.equal(app.timers.size, 0);
  assert.equal(app.window.listeners.blur.length, 0);
  assert.equal(app.calls.at(-1)[1].event.kind, 'release');
  await app.tick(2000);
  assert.equal(captures, 2);
});

test('closing an in-flight game view cannot render or schedule a stale frame', async () => {
  let entered, resume;
  const waiting = new Promise(resolve => { entered = resolve; });
  const gate = new Promise(resolve => { resume = resolve; });
  const app = desktop({}, { snapshot: snapshot(1, 1, [1, 2, 3, 255]), snapshotWait: async () => { entered(); await gate; } });
  await app.api('game_view', {}); await waiting;
  await app.api('game_view_close', {});
  resume(); await flush();
  assert.equal(app.elements.find(el => el.tag === 'canvas').image, undefined);
  assert.equal(app.timers.size, 0);
});

test('closing a pending view prevents a stale ready status from opening a dialog', async () => {
  let entered, resume;
  const waiting = new Promise(resolve => { entered = resolve; });
  const gate = new Promise(resolve => { resume = resolve; });
  const app = desktop({}, { statusWait: async () => { entered(); await gate; } });
  const opening = app.api('game_view', {});
  await waiting;
  await app.api('game_view_close', {});
  resume();
  assert.match((await opening).error, /취소/);
  assert.equal(app.elements.some(el => el.tag === 'dialog'), false);
  assert.equal(app.calls.some(([name]) => name === 'desktop_snapshot'), false);
});

test('private manual input forwards holds and normalized clicks and releases on blur and close', async () => {
  const app = desktop({}, { snapshot: snapshot(2, 1, [1, 2, 3, 255, 4, 5, 6, 255]) });
  await app.api('game_view', {}); await flush();
  const canvas = app.elements.find(el => el.tag === 'canvas');
  canvas.emit('keydown', { code: 'KeyW' });
  canvas.emit('keydown', { code: 'KeyW', repeat: true });
  await flush();
  let events = app.calls.filter(([name]) => name === 'desktop_manual').map(([, payload]) => payload);
  assert.deepEqual(events, [{ generation: 0, event: { kind: 'key', vk: 87, down: true } }]);
  canvas.emit('keyup', { code: 'KeyW' });
  canvas.emit('pointerdown', { button: 0, clientX: 70, clientY: 105, pointerId: 1 });
  canvas.emit('pointerup', { button: 0, clientX: 70, clientY: 105, pointerId: 1 });
  await flush();
  events = app.calls.filter(([name]) => name === 'desktop_manual').map(([, payload]) => payload.event);
  assert.deepEqual(events.slice(1), [{ kind: 'key', vk: 87, down: false }, { kind: 'pointer', x: 0.25, y: 0.75, right: false, down: true }, { kind: 'pointer', x: 0.25, y: 0.75, right: false, down: false }]);
  const beforeModifiers = app.calls.length;
  canvas.emit('keydown', { code: 'ShiftLeft' });
  canvas.emit('keydown', { code: 'ShiftRight' });
  canvas.emit('keyup', { code: 'ShiftLeft' });
  await flush();
  assert.deepEqual(app.calls.slice(beforeModifiers).map(([, p]) => p.event), [{ kind: 'key', vk: 0x10, down: true }]);
  canvas.emit('keyup', { code: 'ShiftRight' });
  for (const code of ['Digit3', 'ArrowLeft', 'Space', 'Escape', 'Enter', 'Tab', 'Backspace', 'ControlLeft', 'AltLeft']) {
    canvas.emit('keydown', { code }); canvas.emit('keyup', { code });
    await flush();
  }
  await flush();
  assert.deepEqual(app.calls.slice(beforeModifiers).filter(([, p]) => p.event.kind === 'key' && p.event.down).map(([, p]) => p.event.vk), [0x10, 0x33, 0x25, 0x20, 0x1b, 0x0d, 0x09, 0x08, 0x11, 0x12]);
  app.window.emit('blur'); await flush();
  assert.equal(app.calls.at(-1)[1].event.kind, 'release');
  await app.api('session_stop', {}); await flush();
  assert.equal(app.elements.find(el => el.tag === 'dialog').removed, true);
  assert.equal(app.timers.size, 0);
});

test('right drag scales relative look to the private viewport and limits input frequency', async () => {
  const app = desktop({}, { snapshot: snapshot(400, 200, new Uint8Array(400 * 200 * 4)) });
  await app.api('game_view', {}); await flush();
  const canvas = app.elements.find(el => el.tag === 'canvas');
  canvas.emit('pointerdown', { button: 2, clientX: 50, clientY: 50, pointerId: 1 });
  canvas.emit('pointermove', { clientX: 60, clientY: 55 });
  canvas.emit('pointermove', { clientX: 70, clientY: 60 });
  await app.tick(33);
  assert.equal(app.calls.filter(([name, p]) => name === 'desktop_manual' && p.event.kind === 'look').length, 0);
  await app.tick(1);
  const look = app.calls.filter(([name, p]) => name === 'desktop_manual' && p.event.kind === 'look');
  assert.deepEqual(look.map(([, p]) => p.event), [{ kind: 'look', dx: 40, dy: 20 }]);
  await app.api('game_view_close', {}); await flush();
});

test('slow manual acknowledgments coalesce motion and prioritize mouse-up or release without a motion tail', async () => {
  for (const end of ['pointerup', 'blur']) {
    const acknowledgments = [];
    let inFlight = 0, maximumInFlight = 0;
    const app = desktop({}, {
      snapshot: snapshot(400, 200, new Uint8Array(400 * 200 * 4)),
      manualWait: async () => {
        maximumInFlight = Math.max(maximumInFlight, ++inFlight);
        await new Promise(resolve => acknowledgments.push(resolve));
        --inFlight;
      },
    });
    await app.api('game_view', {}); await flush();
    const canvas = app.elements.find(el => el.tag === 'canvas');
    canvas.emit('pointerdown', { button: 2, clientX: 50, clientY: 50, pointerId: 1 });
    acknowledgments.shift()(); await flush();
    canvas.emit('pointermove', { clientX: 60, clientY: 55 });
    await app.tick(34);
    assert.equal(inFlight, 1, 'a look command is awaiting its backend acknowledgment');
    for (let i = 0; i < 100; i++) {
      canvas.emit('pointermove', { clientX: 61 + i, clientY: 56 + i });
      await app.tick(34);
    }
    let manual = app.calls.filter(([name]) => name === 'desktop_manual');
    assert.equal(manual.length, 2, 'pending motion never starts concurrent invokes');
    if (end === 'pointerup') canvas.emit('pointerup', { button: 2, clientX: 160, clientY: 155, pointerId: 1 });
    else app.window.emit('blur');
    acknowledgments.shift()(); await flush();
    manual = app.calls.filter(([name]) => name === 'desktop_manual');
    assert.equal(manual.length, 3, 'the release follows the single in-flight motion');
    assert.equal(manual.at(-1)[1].event.kind, end === 'pointerup' ? 'pointer' : 'release');
    if (end === 'pointerup') assert.equal(manual.at(-1)[1].event.down, false);
    acknowledgments.shift()(); await flush();
    await app.tick(1000);
    assert.equal(app.calls.filter(([name]) => name === 'desktop_manual').length, 3, 'no old look executes after release');
    assert.equal(maximumInFlight, 1);
    await app.api('game_view_close', {});
    while (acknowledgments.length) { acknowledgments.shift()(); await flush(); }
  }
});

test('manual key backlog is bounded and blur replaces pending down/up pairs with one release', async () => {
  const acknowledgments = [];
  const app = desktop({}, {
    snapshot: snapshot(1, 1, [1, 2, 3, 255]),
    manualWait: () => new Promise(resolve => acknowledgments.push(resolve)),
  });
  await app.api('game_view', {}); await flush();
  const canvas = app.elements.find(el => el.tag === 'canvas');
  for (let i = 0; i < 100; i++) {
    canvas.emit('keydown', { code: 'KeyW' }); canvas.emit('keyup', { code: 'KeyW' });
  }
  app.window.emit('blur');
  assert.equal(app.calls.filter(([name]) => name === 'desktop_manual').length, 1);
  acknowledgments.shift()(); await flush();
  const manual = app.calls.filter(([name]) => name === 'desktop_manual');
  assert.equal(manual.length, 2);
  assert.deepEqual(manual.at(-1)[1].event, { kind: 'release' });
  acknowledgments.shift()(); await flush();
  await app.api('game_view_close', {});
  while (acknowledgments.length) { acknowledgments.shift()(); await flush(); }
});

test('active macro preview is read-only and stale manual input is not retried in a new generation', async () => {
  const app = desktop({}, { snapshot: snapshot(1, 1, [1, 2, 3, 255]) });
  app.status.dua.running = true; app.status.phase = 'reeling';
  await app.api('game_view', {}); await flush();
  const canvas = app.elements.find(el => el.tag === 'canvas');
  assert.ok(canvas.image, 'macro-active snapshots remain visible');
  canvas.emit('keydown', { code: 'KeyW' });
  canvas.emit('pointerdown', { button: 0, clientX: 50, clientY: 50 });
  await flush();
  assert.equal(app.calls.some(([name]) => name === 'desktop_manual'), false);
  app.status.dua.running = false; app.status.phase = 'idle'; app.status.generation++;
  await app.tick(500);
  const generation = app.status.generation;
  app.status.generation++;
  canvas.emit('keydown', { code: 'KeyA' }); await flush();
  const keys = app.calls.filter(([name, p]) => name === 'desktop_manual' && p.event.kind === 'key');
  assert.equal(keys.length, 1);
  assert.equal(keys[0][1].generation, generation);
  await app.tick(500);
  assert.equal(app.calls.filter(([name, p]) => name === 'desktop_manual' && p.event.kind === 'key').length, 1);
  await app.api('game_view_close', {}); await flush();
});

test('legacy timed navigation cannot be executed through the desktop bridge', async () => {
  const app = desktop({});
  for (const action of ['move_base', 'move_place', 'move_pick', 'move_test', 'move_arrive', 'sell_test']) {
    assert.match((await app.api(action, {})).error, /지도 기반 이동/);
  }
  assert.equal(app.calls.length, 0);
});

test('desktop fishing scope labels exclude selling while legacy UI text stays unchanged', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const html = fs.readFileSync(path.join(__dirname, '../src/web/index.html'), 'utf8');
  const labels = [...html.matchAll(/data-desktop-text="([^"]+)"[^>]*>([^<]+)/g)].map(([, desktop, legacy]) => ({
    dataset: { desktopText: desktop }, textContent: legacy,
  }));
  assert.equal(labels.length, 2);
  const original = labels.map(el => el.textContent);
  const context = { window: {}, document: { querySelectorAll: () => labels }, setText: (el, text) => { el.textContent = text; } };
  vm.runInNewContext(source.slice(source.indexOf('function applyDesktopText('), source.indexOf('function syncMainTiles(')) + '\nglobalThis.apply = applyDesktopText;', context);
  context.apply();
  assert.deepEqual(labels.map(el => el.textContent), original);
  assert.match(original[1], /가득 차면 팔고 돌아옴/);
  context.window.acruxDesktopApi = () => {};
  context.apply();
  assert.equal(labels[0].textContent, '던지기 · 릴링');
  assert.match(labels[1].textContent, /가득 차면 중지/);
  assert.match(labels[1].textContent, /이동과 판매는 준비 중/);
});

test('private session uses app controls while the legacy Python UI retains shortcuts', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const hintText = '내 서버에서 레어 바이옴이 시작되면 포션 사용 · 정지: F7';
  const measureText = '시간 재는 중 · 도착하면 F6을 누르세요.';
  const measureLabel = 'Measuring time · press [Arrived] when you arrive.';
  const hint = { textContent: hintText };
  const movement = {}, statusText = {};
  const elements = new Map();
  const labels = new Map();
  const translations = new Map([
    [hintText, 'Uses potions when a rare biome starts in your server · stop: F7'],
    ['앱의 시작·중지 버튼', 'Start/Stop button in the app'],
    ['앱의 중지 버튼', 'Stop button in the app'],
    ['도착', 'Arrived'],
    [measureText, 'Measuring time · press F6 when you arrive.'],
  ]);
  const context = {
    window: { acruxDesktopApi() {} }, config: { macro_on: false }, FEATS: [],
    document: {
      querySelector: () => ({}),
      querySelectorAll: selector => selector === '.move-state' ? [movement] : selector === '[data-status-text]' ? [statusText] : [hint],
      createElement: () => ({ children: [{}, {}] }),
    },
    $: id => { if (!elements.has(id)) elements.set(id, {}); return elements.get(id); },
    setText: (el, text) => { el.textContent = text; }, setData() {},
    setTile: (name, on, label) => labels.set(name, label),
    tile: () => ({ classList: { remove() {} } }),
    bio: () => ({}), syncFeatItems() {}, Mods: { emit: () => [] },
    rerenderMove() {}, LOG_SINCE: 0, fmtTime: () => '',
    I18N: { source: el => el.textContent, t: text => translations.get(text) ?? text },
  };
  vm.runInNewContext(source.slice(source.indexOf('const macroFeatures ='), source.indexOf('function setStatus('))
    + '\nglobalThis.ui = { updateDua, updateSession, syncMainTiles, shortcutText };', context);
  vm.runInNewContext(source.slice(source.indexOf('function setStatus('), source.indexOf('// ---------------------------------------------------------------- 주기적으로 엔진 상태 받기'))
    + '\nglobalThis.ui.setStatus = setStatus;', context);
  vm.runInNewContext('let lastMove = null;\n' + source.slice(source.indexOf('function updateMove('), source.indexOf('// 위치는 서버가 이미 저장함'))
    + '\nglobalThis.ui.updateMove = updateMove;', context);
  vm.runInNewContext(source.slice(source.indexOf('function updateMfish('), source.indexOf("$('mfishStop').addEventListener"))
    + '\nglobalThis.ui.updateMfish = updateMfish;', context);
  vm.runInNewContext(source.slice(source.indexOf('function addLog('), source.indexOf("$('clearLog').addEventListener"))
    + '\nglobalThis.ui.addLog = addLog;', context);
  const status = { hotkeys: false, running: false, route: null, headlessTicks: 0, screenshotFrames: 0, transitions: 0 };
  context.ui.updateSession({ supported: true, state: 'stopped', message: 'Open the game' });
  assert.equal(elements.get('desktopSessionMessage').textContent, 'Open the game');
  assert.equal(elements.get('desktopSessionStart').disabled, false);
  assert.equal(elements.get('desktopGameViewOpen').disabled, true);
  context.ui.updateSession({ supported: true, state: 'ready', message: 'Ready' });
  assert.equal(elements.get('desktopSessionStart').disabled, true);
  assert.equal(elements.get('desktopSessionStop').disabled, false);
  assert.equal(elements.get('desktopGameViewOpen').disabled, false);
  context.ui.updateDua(status);
  context.ui.syncMainTiles();
  assert.equal(labels.get('macro'), '시작');
  assert.equal(elements.get('duaStopMethod').textContent, '앱의 중지 버튼');
  assert.equal(hint.textContent, 'Uses potions when a rare biome starts in your server · stop: Stop button in the app');
  assert.equal(context.ui.shortcutText('F3 / F6 / F7'), 'Start/Stop button in the app / [Arrived] / Stop button in the app');
  context.ui.updateMove({ running: true, msg: measureText });
  context.ui.updateMove({ running: true, msg: measureText });
  assert.equal(movement.textContent, measureLabel);
  context.ui.setStatus(['working', measureText]);
  assert.equal(statusText.textContent, measureLabel);
  context.ui.updateMfish({ running: true, msg: measureText });
  assert.equal(elements.get('mfishState').textContent, measureLabel);
  const log = { children: [], scrollHeight: 0, scrollTop: 0, clientHeight: 0, appendChild(row) { this.children.push(row); } };
  elements.set('log', log);
  context.ui.addLog({ time: 1, msg: measureText });
  assert.equal(log.children[0].children[1].textContent, measureLabel);
  context.config.macro_on = true;
  vm.runInNewContext('lastMfish = { running: true, msg: ' + JSON.stringify(measureText) + ' };', context);
  context.ui.syncMainTiles();
  assert.equal(labels.get('macro'), '중지');
  assert.equal(elements.get('mtMacro').textContent, measureLabel);
  context.ui.updateDua({ ...status, hotkeys: true });
  context.ui.syncMainTiles();
  assert.equal(labels.get('macro'), '중지 · F3');
  assert.equal(hint.textContent, hintText);
  context.ui.updateMove({ running: true, msg: measureText });
  assert.equal(movement.textContent, measureText);
  delete context.window.acruxDesktopApi;
  context.ui.updateDua(undefined);
  assert.equal(elements.get('duaPanel').hidden, true);
  assert.equal(context.ui.shortcutText('F3 / F6 / F7'), 'F3 / F6 / F7');
  context.ui.updateMove({ running: true, msg: measureText });
  assert.equal(movement.textContent, measureText);
});

test('Discord bridge keeps the four public contracts and never stores account credentials in UI preferences', async () => {
  const app = desktop({ discord: { token: 'private-backend-only', name: 'old' } });
  assert.deepEqual(await app.api('discord_state', {}), { linked: false, name: '', code: null, error: '' });
  assert.deepEqual({ ...await app.api('discord_login', {}) }, { ok: true, code: 'ABCD1234' });
  assert.deepEqual({ ...await app.api('discord_cancel', {}) }, { ok: true, code: null });
  app.discord.linked = true; app.discord.name = 'User';
  assert.equal((await app.api('discord_state', {})).name, 'User');
  assert.deepEqual({ ...await app.api('discord_logout', {}) }, { ok: true, code: null });
  assert.equal(app.stored().discord, undefined);
  assert.equal(app.calls.some(([name]) => name === 'control' || name === 'desktop_start'), false);
  const failed = desktop({}, { discordError: '서버에 연결할 수 없음' });
  assert.deepEqual({ ...await failed.api('discord_login', {}) }, { error: '서버에 연결할 수 없음' });
});

test('PC Discord UI renders pending code, account name and unlink while keeping older Python UI unchanged', async () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const app = desktop({});
  const elements = new Map();
  const panels = [{ style: {}, hidden: true }, { style: {}, hidden: true }];
  const $ = id => {
    if (!elements.has(id)) elements.set(id, { listeners: {}, classList: { toggle() {} }, addEventListener(name, fn) { this.listeners[name] = fn; } });
    return elements.get(id);
  };
  let supported = true;
  const context = {
    $, document: { querySelectorAll: () => panels }, current: 'acrux',
    setText: (el, text) => { el.textContent = text; }, I18N: { t: text => text },
    api: (name, args) => supported ? app.api(name, args) : Promise.resolve({ error: 'old Python backend' }),
    setTimeout: () => 1, clearTimeout() {}, toast() {},
  };
  const start = source.indexOf('// Account linking is independent');
  vm.runInNewContext(source.slice(start, source.indexOf("$('acFolder').addEventListener", start))
    + '\nglobalThis.dcUi = { refreshDiscord };', context);
  await context.dcUi.refreshDiscord();
  assert.equal(panels[0].hidden, false);
  await $('dcBtn').listeners.click();
  assert.equal($('dcHint').textContent.includes('ABCD-1234'), true);
  assert.equal($('dcBtn').textContent, '취소');
  assert.equal(app.discord.code, 'ABCD1234');
  await $('dcBtn').listeners.click();
  assert.equal(app.discord.code, null);
  app.discord.linked = true; app.discord.name = '<img src=x onerror=bad()>';
  await context.dcUi.refreshDiscord();
  assert.equal($('dcName').textContent, app.discord.name);
  assert.equal($('dcName').innerHTML, undefined);
  assert.equal($('dcBtn').textContent, '연결 끊기');
  await $('dcBtn').listeners.click();
  assert.equal(app.discord.linked, false);
  supported = false;
  await context.dcUi.refreshDiscord();
  assert.equal(panels.every(panel => panel.hidden && panel.style.display === 'none'), true);
});

test('mobile Discord settings invoke the same four commands and render only public state', async () => {
  const source = fs.readFileSync(path.join(__dirname, 'main.ts'), 'utf8');
  const declarations = source.slice(source.indexOf('interface DiscordState'), source.indexOf('function message('));
  const functions = source.slice(source.indexOf('function applyDiscord()'), source.indexOf('function statsText('));
  const compiled = ts.transpileModule(declarations + functions + '\nglobalThis.dcUi = { refreshDiscord, changeDiscord };', {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const app = desktop({});
  const elements = new Map(['discord-name', 'discord-state', 'discord-action'].map(id => [id, {}]));
  const context = {
    document: { hidden: false, getElementById: id => elements.get(id) }, page: 'settings',
    invoke: app.invoke, setText: (id, text) => { elements.get(id).textContent = text; },
    t: text => text, setTimeout: () => 1, clearTimeout() {}, message() {},
  };
  vm.runInNewContext(compiled, context);
  await context.dcUi.refreshDiscord();
  await context.dcUi.changeDiscord();
  assert.equal(elements.get('discord-state').textContent.includes('ABCD-1234'), true);
  assert.equal(elements.get('discord-action').textContent, '취소');
  await context.dcUi.changeDiscord();
  app.discord.linked = true; app.discord.name = '<script>account</script>';
  await context.dcUi.refreshDiscord();
  assert.equal(elements.get('discord-name').textContent, app.discord.name);
  assert.equal(elements.get('discord-name').innerHTML, undefined);
  await context.dcUi.changeDiscord();
  assert.equal(app.discord.linked, false);
  const before = app.calls.length;
  context.document.hidden = true;
  await context.dcUi.refreshDiscord();
  assert.equal(app.calls.length, before);
  assert.deepEqual(new Set(app.calls.map(([name]) => name)), new Set(['discord_state', 'discord_login', 'discord_cancel', 'discord_logout']));
  assert.equal(JSON.stringify(app.stored()).includes('account'), false);
});

test('Discord account labels, pending code and backend errors translate without changing codes or names', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/i18n.js'), 'utf8');
  const body = { nodeType: 1, querySelectorAll: () => [], matches: () => false, closest: () => null, hasAttribute: () => false };
  const context = { document: { body, documentElement: {}, createTreeWalker: () => ({ nextNode: () => false }) }, NodeFilter: { SHOW_TEXT: 4 } };
  vm.runInNewContext(source + '\nglobalThis.translations = I18N;', context);
  const mobile = new Map(fs.readFileSync(path.join(__dirname, '../android/app/src/main/assets/i18n/pc.tsv'), 'utf8')
    .trim().split('\n').map(line => { const [ko, ...other] = line.split('\t'); return [ko, other]; }));
  for (const lang of ['en', 'ja']) {
    context.translations.set(lang);
    const login = context.translations.t('디스코드 로그인');
    assert.equal(login, lang === 'en' ? 'Discord login' : 'Discord ログイン');
    assert.equal(context.translations.t('브라우저에서 [연결] 을 눌러 주세요 · 코드 ABCD-1234').includes('ABCD-1234'), true);
    assert.equal(context.translations.t('디스코드 연결됨 — 계정이름').endsWith('계정이름'), true);
    const error = context.translations.t('연결 시간이 지났습니다. 다시 연결해 주세요.');
    assert.doesNotMatch(error, /[가-힣]/);
    assert.equal(mobile.get('디스코드 로그인')[lang === 'en' ? 0 : 1], login);
    assert.equal(mobile.get('브라우저에서 [연결] 을 눌러 주세요 · 코드 {0}')[lang === 'en' ? 0 : 1].includes('{0}'), true);
  }
});

test('saving cannot restore retired settings and preserves the Extras tab', async () => {
  const app = desktop({ menu_tab: 'extra' });
  await app.api('set_config', { patch: {
    snipe: { enabled: true }, prime: { key: 'retired' },
    pop: { match_threshold: 80, delays: { search: 0.1, after_join: 7 }, templates: {} },
    ret: { ps_link: 'own', start_wait: 8, after_macro: true, steps: [] },
  } });
  const config = app.stored();
  assert.ok(!('snipe' in config) && !('prime' in config));
  assert.equal(config.menu_tab, 'extra');
  assert.deepEqual(config.pop, { match_threshold: 80, delays: { search: 0.1 } });
  assert.deepEqual(config.ret, { ps_link: 'own', start_wait: 8 });
});

test('desktop assets contain no retired screens or controls', () => {
  const tauri = JSON.parse(fs.readFileSync(path.join(__dirname, '../src-tauri/tauri.conf.json'), 'utf8'));
  assert.doesNotMatch(tauri.app.windows[0].title, /snipe|sniping/i);
  for (const filename of ['app.js', 'index.html', 'style.css', 'i18n.js']) {
    const source = fs.readFileSync(path.join(__dirname, '../src/web', filename), 'utf8');
    assert.doesNotMatch(source, /스나이|スナイ|Prime Snipe|data-mpos-link|setPopLink|SNIPE_BIOMES|STAY_BIOMES|모든 채널 감시|디버그 포트/);
    if (filename.endsWith('.js')) new vm.Script(source, { filename });
  }
});
