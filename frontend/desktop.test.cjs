const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const ts = require('typescript');
const portableStub = {
  PortableMods: class { async start() {} list(items) { return items; } render() {} },
  pickModPackage: async () => ({ cancel: true }),
};

async function flush() { for (let i = 0; i < 24; i++) await Promise.resolve(); }

function mobile(options = {}) {
  const calls = [], listeners = new Map(), elements = new Map();
  const stats = { success: 0, junk: 0, fail: 0, full: 0 };
  const current = {
    platform: 'android', version: '0.1.0', phase: 'idle', message: '대기 중', calibrated: false,
    captureReady: true, inputReady: true, gameAvailable: false, generation: 7, stats, totals: stats,
    tuning: { biteSeconds: 60, leadMs: 0, targetPercent: 0, adaptive: true, tapAbove: true },
    dua: { supported: true, background: false, running: false, route: null, reason: 'DUA 대기 중', headlessTicks: 0, screenshotFrames: 0, transitions: 0 },
  };
  const element = id => {
    if (!elements.has(id)) elements.set(id, {
      style: {}, dataset: {}, classList: { toggle() {} }, children: [],
      addEventListener() {}, querySelectorAll: () => [], setAttribute() {},
      scrollTo() {}, scrollTop: 0, showModal() { this.open = true; }, close() { this.open = false; },
    });
    return elements.get(id);
  };
  const mobileConfig = structuredClone(JSON.parse(fs.readFileSync(path.join(__dirname, 'desktop-defaults.json'), 'utf8')));
  mobileConfig.mfish.enabled = true;
  let nativeMobileConfig = mobileConfig;
  const invoke = async (name, payload) => {
    calls.push([name, structuredClone(payload)]);
    if (name === 'status') return structuredClone(current);
    if (name === 'macro_settings') return structuredClone(nativeMobileConfig);
    if (name === 'save_macro_settings') { if (options.configSaveWait) await options.configSaveWait; nativeMobileConfig = structuredClone(payload.config); return structuredClone(nativeMobileConfig); }
    if (name === 'android_input_status') return { enabled: true, selected: true };
    if (name === 'automation_action') { if (options.automationError) throw new Error(options.automationError); return { ok: true, queued: true, message: '실제 작업 요청' }; }
    if (name === 'service_action') return { available: true, ready: payload.action === 'update_download', message: '업데이트 상태' };
    if (name === 'pick_macro_position') return { pos: [.2, .3], config: nativeMobileConfig };
    if (name === 'dua_start') {
      const staleReply = structuredClone(current);
      Object.assign(staleReply.dua, { running: true, route: 'headless' });
      if (options.startWait) await options.startWait;
      if (options.staleStartReply) return staleReply;
      if (payload.generation !== current.generation) throw new Error('이전 실행 요청이 취소됐습니다.');
      current.generation++;
      Object.assign(current.dua, { running: true, route: 'headless', reason: 'Roblox가 앞에 오기를 기다립니다.' });
      return structuredClone(current);
    }
    if (name === 'dua_stop') {
      current.generation++;
      current.phase = 'idle';
      Object.assign(current.dua, { running: false, route: null, reason: '중지됨' });
      return structuredClone(current);
    }
    if (name === 'save_tuning') {
      current.tuning = payload.tuning;
      const reply = structuredClone(current);
      if (options.saveWait) await options.saveWait;
      return reply;
    }
    if (name === 'biome_transport') return { connection: 'off', testOutput: '', testUntil: 0 };
    if (name === 'online_status') return { online: null, share: true };
    if (name === 'save_online') return;
    if (name === 'android_setup') return;
    throw new Error(`Unexpected mobile command ${name}`);
  };
  const compile = source => ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, esModuleInterop: true },
  }).outputText;
  const engine = { exports: {}, require: name => {
    assert.equal(name, '@tauri-apps/api/core');
    return { invoke };
  } };
  vm.runInNewContext(compile(fs.readFileSync(path.join(__dirname, 'engine.ts'), 'utf8')), engine);
  const modules = {
    './dua': {}, './portable-mods': portableStub,
    './engine': engine.exports,
    './desktop-defaults.json': JSON.parse(fs.readFileSync(path.join(__dirname, 'desktop-defaults.json'), 'utf8')),
    './biome': { defaultBiome: {}, biomeStatus: async () => ({ settings: {}, logs: [] }) },
    '@tauri-apps/api/core': { invoke },
  };
  for (const name of ['pc', 'mobile']) modules[`../android/app/src/main/assets/i18n/${name}.tsv?raw`] = fs.readFileSync(path.join(__dirname, `../android/app/src/main/assets/i18n/${name}.tsv`), 'utf8');
  const context = {
    exports: {}, require: name => { assert.ok(name in modules, `Unexpected mobile module ${name}`); return modules[name]; },
    document: { hidden: true, documentElement: {}, getElementById: element, querySelectorAll: () => [],
      addEventListener: (name, fn) => listeners.set(name, fn) },
    window: { setInterval() {}, setTimeout: () => 1 }, navigator: { language: 'ko' },
    localStorage: { getItem: () => null, setItem() {} }, setTimeout: () => 1, clearTimeout() {}, structuredClone,
  };
  const source = fs.readFileSync(path.join(__dirname, 'main.ts'), 'utf8');
  vm.runInNewContext(compile(source + '\nglobalThis.mobileUi = {'
    + ' changeMacro, perform, applyStatus, home, move, guideSteps, loadMacro, macroSettings, saveMacro, startGuide, moveGuide,'
    + ' get guide() { return guide; }, get current() { return current; }, get saves() { return numberSaves; }, get macroSaves() { return macroSaves; }, get macro() { return macro; },'
    + ' setCurrent(value) { current = value; applyStatus(); },'
    + ' setLanguage(value) { overlay.lang = value; },'
    + ' setBusy(value) { busy = value; } };'), context);
  context.document.hidden = false;
  context.mobileUi.setCurrent(structuredClone(current));
  return { ui: context.mobileUi, current, calls, element, listeners, document: context.document };
}

function desktop(saved, options = {}) {
  const preferencesKey = 'acrux.desktop.preferences';
  const storage = new Map([[preferencesKey, JSON.stringify(saved)]]);
  const errors = [], calls = [];
  let nativeConfig;

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
  window.confirm = () => options.confirm !== false;
  const stats = { success: 0, junk: 0, fail: 0, full: 0 };
  const status = {
    phase: 'idle', positions: null, platform: options.platform ?? 'linux', captureReady: options.background !== false, inputReady: options.background !== false,
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
    './dua': {}, './portable-mods': portableStub,
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
      if (command === 'macro_settings') {
        if (options.loadWait) await options.loadWait;
        if (!nativeConfig) nativeConfig = JSON.parse(storage.get(preferencesKey));
        return structuredClone(nativeConfig);
      }
      if (command === 'save_macro_settings') {
        if (options.configSaveWait) await options.configSaveWait;
        if (Number(payload.config.mfish?.bite_max) > 600) throw new Error('낚시 설정의 범위를 확인하세요.');
        if (status.dua.running || status.phase !== 'idle') throw new Error('설정을 바꾸기 전에 실행 중인 작업을 중지하세요.');
        nativeConfig = structuredClone(payload.config);
        return structuredClone(nativeConfig);
      }
      if (command === 'service_action') {
        if (options.serviceError) throw new Error(options.serviceError);
        if (payload.action === 'mods_list') return { mods: options.mods ?? [] };
        if (payload.action === 'runtime_metadata') return structuredClone(options.metadata ?? { logs: [], stats: {} });
        if (payload.action === 'mods_hook' && options.hook) return options.hook(payload.payload.event, payload.payload.args);
        return { ok: true, available: true, ready: payload.action === 'update_download' };
      }
      if (command === 'automation_action') {
        if (options.automationError) throw new Error(options.automationError);
        assert.equal(payload.generation, status.generation);
        return { ok: true, queued: true };
      }
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
        status.session.state = 'ready'; status.dua.background = true; status.captureReady = true; status.inputReady = true;
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
      if (command === 'pick_macro_position') {
        if (options.pickWait) await options.pickWait;
        if (options.pickError) throw new Error(options.pickError);
        assert.equal(payload.generation, status.generation);
        const pos = payload.region ? [.1, .2, .7, .8] : [.2, .3];
        nativeConfig[payload.feat][payload.key] = pos;
        return { pos, config: structuredClone(nativeConfig) };
      }
      if (command === 'desktop_cancel_pick') { options.cancelPick?.(); return; }
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

test('rejected fishing settings do not poison a later unrelated save', async () => {
  const app = desktop({});
  const initial = await app.api('state', {});
  const rejected = await app.api('set_config', { patch: { mfish: { ...initial.config.mfish, bite_max: 9999 } } });
  assert.match(rejected.error, /범위/);
  assert.equal(rejected.config.mfish.bite_max, initial.config.mfish.bite_max);
  const next = await app.api('set_config', { patch: { mitem: { ...initial.config.mitem, enabled: true } } });
  assert.equal(next.error, undefined);
  assert.equal(next.config.mitem.enabled, true);
  assert.notEqual(app.calls.filter(([name]) => name === 'save_macro_settings').at(-1)[1].config.mfish.bite_max, 9999);
});

test('initial state failure still returns usable configuration and an error', async () => {
  const app = desktop({}, { serviceError: '모드 서비스 연결 실패' });
  const result = await app.api('state', {});
  assert.match(result.error, /모드 서비스/);
  assert.ok(result.config);
  assert.ok(Array.isArray(result.mods));
});

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
  assert.deepEqual(app.calls.filter(([name]) => name === 'desktop_start').map(([name]) => name), ['desktop_start']);
  assert.deepEqual(app.calls.find(([name]) => name === 'desktop_start')[1], { features: ['mfish'], generation: 0 });
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

test('running config edits stop, save and restart exactly once while disabled features are reflected', async () => {
  const app = desktop({ mfish: { enabled: true }, mitem: { enabled: true } });
  await app.api('set_config', { patch: { macro_on: true } });
  await app.api('set_config', { patch: { mfish: { enabled: true, sell_max: 80 } } });
  const sequence = app.calls.map(([name]) => name).filter(name => ['desktop_start', 'control', 'save_macro_settings'].includes(name));
  assert.deepEqual(sequence, ['desktop_start', 'control', 'save_macro_settings', 'desktop_start']);
  assert.equal(app.status.dua.running, true);
  await app.api('set_config', { patch: { mfish: { enabled: false } } });
  assert.deepEqual(app.calls.filter(([name]) => name === 'desktop_start').at(-1)[1].features, ['mitem']);
});

test('editing during manual calibration stops before saving and does not restart a macro', async () => {
  const app = desktop({ mfish: { enabled: false } });
  app.status.phase = 'calibrating';
  await app.api('set_config', { patch: { mfish: { enabled: true } } });
  assert.equal(app.status.phase, 'idle');
  const meaningful = app.calls.filter(([name]) => ['control', 'save_macro_settings', 'desktop_start'].includes(name));
  assert.deepEqual(meaningful.map(([name]) => name), ['control', 'save_macro_settings']);
  await app.api('set_config', { patch: { macro_on: false } });
  assert.equal(app.calls.filter(([name]) => name === 'desktop_start').length, 0);
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

test('Windows position and region settings use live game selection and keep the saved native config', async () => {
  for (const [action, payload, expected] of [
    ['mpos_point', { feat: 'base', key: 'chat_pos' }, [.2, .3]],
    ['mpos_region', { feat: 'mfish', key: 'panel_region' }, [.1, .2, .7, .8]],
    ['play_pos', { which: 'skip' }, [.2, .3]],
  ]) {
    const app = desktop({}, { platform: 'windows' });
    const result = await app.api(action, payload);
    assert.deepEqual(result.pos, expected);
    const pick = app.calls.find(([name]) => name === 'pick_macro_position')[1];
    assert.equal(pick.region, action === 'mpos_region');
    assert.equal(pick.generation, 0);
    assert.deepEqual(app.stored()[pick.feat][pick.key], expected);
    assert.equal(app.elements.some(el => el.tag === 'dialog'), false);
    assert.equal(app.calls.some(([name]) => name === 'desktop_snapshot' || name === 'save_macro_settings'), false);
  }
});

test('Windows live selection cancels through the native owner without persisting a coordinate', async () => {
  let cancel;
  const wait = new Promise((_, reject) => { cancel = () => reject(new Error('위치 지정을 취소했습니다.')); });
  const app = desktop({}, { platform: 'windows', pickWait: wait, cancelPick: () => cancel() });
  const pending = app.api('mpos_point', { feat: 'base', key: 'chat_pos' });
  await flush();
  await app.api('pick_cancel', {});
  assert.match((await pending).error, /취소/);
  assert.equal(app.stored().base.chat_pos, null);
  assert.equal(app.calls.filter(([name]) => name === 'desktop_cancel_pick').length, 1);
});

test('place actions keep their own errors while disconnected instead of silently opening a session', async () => {
  const app = desktop({}, { background: false, session: { state: 'stopped' }, automationError: '게임 캡처와 입력을 먼저 연결하세요.' });
  for (const action of ['go', 'waypoint_mark', 'waypoint_cancel']) {
    assert.match((await app.api(action, { destination: 'sell' })).error, /먼저 연결/);
  }
  assert.equal(app.calls.some(([name]) => name === 'desktop_session_start'), false);
});

test('snapshot picker rejects invalid dimensions and truncated or extra pixel data before creating a dialog', async () => {
  for (const data of [new ArrayBuffer(4), snapshot(0, 1), snapshot(16384, 16384), snapshot(2, 1, [1, 2, 3, 4]), snapshot(1, 1, [1, 2, 3, 4, 5])]) {
    const app = desktop({}, { snapshot: data });
    const result = await app.api('mpos_point', { feat: 'base', key: 'chat_pos' });
    assert.match(result.error, /게임 화면/);
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
  assert.match(result.error, /게임 열기/);
  const started = await app.api('set_config', { patch: { macro_on: true } });
  assert.equal(started.config.macro_on, false);
  assert.match(app.errors[0], /게임 열기/);
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
  assert.match((await unsupported.api('game_view', {})).error, /게임 열기/);
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

test('navigation and legacy feature APIs reach real native actions with a fresh generation', async () => {
  const app = desktop({});
  for (const action of ['move_base', 'move_place', 'move_pick', 'move_test', 'move_arrive', 'sell_test', 'mpop_test', 'mitem_test', 'mmerch_check', 'mcraft_test', 'mmatch_test', 'mblood_test', 'ret_test', 'play_test', 'ocr_info', 'waypoint_mark']) {
    assert.equal((await app.api(action, { place: 2, name: 'Lucky Potion', destination: 'craft' })).queued, true, action);
    app.status.generation++;
  }
  const navigation = app.calls.filter(([name]) => name === 'automation_action');
  assert.equal(navigation.length, 16);
  assert.equal(navigation.find(([, p]) => p.action === 'move_place')[1].payload.i, 2);
  assert.equal(navigation.filter(([, p]) => p.action === 'waypoint_mark').length, 2);
  assert.equal(app.calls.some(([name]) => name === 'desktop_navigation'), false);
  const failed = desktop({}, { automationError: '화면에서 캐릭터 위치를 확인할 수 없습니다.' });
  assert.match((await failed.api('move_base', {})).error, /캐릭터 위치/);
});

test('desktop fishing labels describe native travel and selling while Python text remains unchanged', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const html = fs.readFileSync(path.join(__dirname, '../src/web/index.html'), 'utf8');
  const labels = [...html.matchAll(/data-desktop-text="([^"]+)"[^>]*>([^<]+)/g)].map(([, desktop, legacy]) => ({ dataset: { desktopText: desktop }, textContent: legacy }));
  const original = labels.map(el => el.textContent);
  const context = { window: {}, document: { querySelectorAll: () => labels }, setText: (el, text) => { el.textContent = text; } };
  vm.runInNewContext(source.slice(source.indexOf('function applyDesktopText('), source.indexOf('function syncMainTiles(')) + '\nglobalThis.apply = applyDesktopText;', context);
  context.apply();
  assert.deepEqual(labels.map(el => el.textContent), original);
  context.window.acruxDesktopApi = () => {};
  context.apply();
  assert.equal(labels[0].textContent, '던지기 · 릴링 · 판매');
  assert.match(labels[1].textContent, /지형.*가득 차면 팔고 돌아옴/);
  assert.doesNotMatch(labels[1].textContent, /준비 중/);
});

test('native movement settings and setup omit legacy reset, reverse-base and timed-route requirements', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const definitions = source.slice(source.indexOf('const MPOS ='), source.indexOf('function renderMpos('));
  const requirements = source.slice(source.indexOf('const FEAT_TASKS ='), source.indexOf('const taskCalDone ='));
  const read = native => {
    const context = { window: { acruxDesktopApi: native }, POP_POS: [], MFISH_POS: [] };
    vm.runInNewContext(definitions + requirements + '\nglobalThis.result = { positions: MPOS, tasks: FEAT_TASKS };', context);
    return context.result;
  };
  const native = read(() => {});
  const base = native.positions.base.tabs.find(([id]) => id === 'move');
  assert.equal(base[3].length, 0);
  assert.match(base[2], /카메라를 자동으로 올려/);
  assert.equal(native.positions.mcraft.tabs.some(([id]) => id === 'revbase'), false);
  for (const feature of ['mfish', 'mcraft', 'mmatch']) {
    assert.equal(native.tasks[feature].some(id => /moveBase|craftRev|Place/.test(id)), false, feature);
    const move = native.positions[feature].tabs.find(([id]) => id === 'move');
    assert.doesNotMatch(move[2], /리셋|기준 장소.*출발|퀘스트 보드|→/);
  }
  const legacy = read(undefined);
  assert.deepEqual([...legacy.positions.base.tabs.find(([id]) => id === 'move')[3]], ['chat_pos', 'collection_pos', 'collection_close']);
  assert.equal(legacy.positions.mcraft.tabs.some(([id]) => id === 'revbase'), true);
  assert.equal(legacy.tasks.mcraft.includes('craftRev'), true);
  assert.equal(legacy.tasks.mmatch.includes('mmatchPlace'), true);
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

test('mobile main start uses DUA with automatic calibration and can wait for Roblox without claiming background input', async () => {
  const app = mobile();
  assert.match(app.ui.home(), /data-action="macro-toggle"/);
  assert.equal(app.current.dua.background, false);
  assert.equal(app.current.gameAvailable, false);
  assert.equal(app.element('macro-toggle').disabled, false);
  app.ui.setBusy(true);
  await app.ui.perform('macro-toggle');
  assert.deepEqual({ ...app.calls.find(([name]) => name === 'dua_start')[1] }, { features: ['mfish'], generation: 7 });
  assert.equal(app.ui.current.dua.running, true);
  assert.equal(app.element('macro-toggle').textContent, '정지');
  assert.match(app.element('dua-guard').textContent, /Roblox 화면에서만 터치 · 다른 앱에서는 대기/);
  assert.match(app.element('cal-state').textContent, /시작하면 자동으로 보정/);
  assert.equal(app.calls.some(([name]) => name === 'control' || name === 'desktop_start'), false);
  await app.ui.perform('macro-toggle');
  assert.equal(app.ui.current.dua.running, false);
  assert.equal(app.calls.filter(([name]) => name === 'dua_stop').length, 1);
});

test('mobile start requires capture and input readiness while stop remains available in a DUA idle handoff', async () => {
  const app = mobile();
  for (const field of ['captureReady', 'inputReady']) {
    app.ui.setCurrent({ ...structuredClone(app.current), [field]: false });
    assert.equal(app.element('macro-toggle').disabled, true);
    await app.ui.changeMacro();
  }
  assert.equal(app.calls.some(([name]) => name === 'dua_start'), false);
  const running = structuredClone(app.current);
  running.dua.running = true;
  running.captureReady = running.inputReady = false;
  app.ui.setCurrent(running);
  assert.equal(app.element('macro-toggle').disabled, false);
  await app.ui.changeMacro();
  assert.equal(app.calls.filter(([name]) => name === 'dua_stop').length, 1);
});

test('mobile stop preempts a pending start and ignores both stale success and generation rejection', async () => {
  for (const staleStartReply of [false, true]) {
    let release;
    const startWait = new Promise(resolve => { release = resolve; });
    const app = mobile({ startWait, staleStartReply });
    const starting = app.ui.perform('macro-toggle');
    assert.equal(app.element('macro-toggle').textContent, '정지');
    await flush();
    await app.ui.perform('macro-toggle');
    release();
    await starting;
    await flush();
    assert.equal(app.ui.current.dua.running, false);
    assert.equal(app.ui.current.generation, 8);
    assert.equal(app.element('macro-toggle').textContent, '시작');
    assert.equal(app.calls.filter(([name]) => name === 'dua_start').length, 1);
    assert.equal(app.calls.filter(([name]) => name === 'dua_stop').length, 1);
  }
});

test('mobile tuning saves cannot block stop or restore stale running state after stop', async () => {
  let release;
  const app = mobile({ saveWait: new Promise(resolve => { release = resolve; }) });
  await app.ui.changeMacro();
  app.listeners.get('input')({ target: { disabled: false, dataset: { tuning: 'leadMs' }, type: 'number', valueAsNumber: 45, validity: { valid: true } } });
  await flush();
  await app.ui.perform('macro-toggle');
  assert.equal(app.ui.current.dua.running, false);
  release();
  await app.ui.saves;
  assert.equal(app.ui.current.dua.running, false);
  assert.equal(app.current.tuning.leadMs, 45);
});

test('mobile DUA renders real route counters, foreground guard and screenshot-led travel controls', async () => {
  const app = mobile();
  const observed = structuredClone(app.current);
  Object.assign(observed.dua, { running: true, route: 'screenshot', reason: '실제 화면 확인', headlessTicks: 17, screenshotFrames: 9, transitions: 4 });
  observed.gameAvailable = true;
  app.ui.setCurrent(observed);
  assert.equal(app.element('dua-state').textContent, '실행 중 · 스크린샷');
  assert.equal(app.element('dua-reason').textContent, '실제 화면 확인');
  assert.equal(app.element('dua-counts').textContent, 'Headless 17 · 스크린샷 9 · 경로 전환 4');
  assert.match(app.element('dua-guard').textContent, /✓ Roblox 입력 가능/);
  for (const lang of ['en', 'ja']) {
    app.ui.setLanguage(lang);
    app.ui.applyStatus();
    for (const id of ['dua-state', 'dua-counts', 'dua-guard', 'cal-state']) assert.doesNotMatch(app.element(id).textContent, /[가-힣]/);
    assert.match(app.element('dua-counts').textContent, /17.*9.*4/);
  }
  app.ui.setLanguage('ko');
  const html = app.ui.move();
  assert.match(html, /data-macro="useMove"/);
  assert.match(html, /data-macro="useSell"/);
  assert.match(html, /현재 장소 기억/);
  assert.doesNotMatch(html, /준비 중|data-navigation=/);
  assert.equal(app.calls.some(([name]) => name === 'navigation_task'), false);
  for (const id of ['move', 'sell']) assert.doesNotMatch(app.ui.guideSteps[id][0].body, /준비 중/);
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
  const main = fs.readFileSync(path.join(__dirname, 'main.ts'), 'utf8');
  const parser = ts.transpileModule(main.slice(main.indexOf('const translations ='), main.indexOf('function restore<'))
    + '\nglobalThis.mobileTranslations = translations;', {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const dictionaries = ['pc', 'mobile'].map(name => fs.readFileSync(path.join(__dirname, `../android/app/src/main/assets/i18n/${name}.tsv`), 'utf8'));
  for (const ending of ['\n', '\r\n']) {
    const [desktopDictionary, mobileDictionary] = dictionaries.map(dictionary => dictionary.replace(/\r?\n/g, ending));
    const mobileContext = { desktopDictionary, mobileDictionary: mobileDictionary + ending + ' 공백 \t English \\nline \t 日本語 ' + ending };
    vm.runInNewContext(parser, mobileContext);
    const mobile = mobileContext.mobileTranslations;
    assert.deepEqual([...mobile.get(' 공백 ')], [' English \nline ', ' 日本語 ']);
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

test('desktop stop preempts a slow native settings save and never resumes the pending macro', async () => {
  let release;
  const app = desktop({ mfish: { enabled: true } }, { configSaveWait: new Promise(resolve => { release = resolve; }) });
  await app.api('set_config', { patch: { macro_on: true } });
  const editing = app.api('set_config', { patch: { mitem: { enabled: true } } });
  await flush();
  assert.equal(app.status.dua.running, false);
  await app.api('mfish_stop', {});
  release();
  await editing;
  assert.equal(app.status.dua.running, false);
  assert.equal(app.calls.filter(([name]) => name === 'desktop_start').length, 1);
  assert.equal(app.stored().mitem.enabled, true);
});

test('concurrent desktop edits save every change and restart once with the final selected features', async () => {
  const app = desktop({ mfish: { enabled: true } });
  await app.api('set_config', { patch: { macro_on: true } });
  const first = app.api('set_config', { patch: { mitem: { enabled: true } } });
  const second = app.api('set_config', { patch: { mcraft: { enabled: true, potion: 'Lucky Potion' } } });
  await Promise.all([first, second]);
  const starts = app.calls.filter(([name]) => name === 'desktop_start');
  assert.equal(starts.length, 2);
  assert.deepEqual(starts.at(-1)[1].features, ['mfish', 'mitem', 'mcraft']);
  assert.equal(app.stored().mitem.enabled, true);
  assert.equal(app.stored().mcraft.potion, 'Lucky Potion');
});

test('desktop service buttons use native management and preserve backend errors', async () => {
  const app = desktop({});
  for (const action of ['mods_list', 'mods_folder', 'mods_toggle', 'mods_remove', 'open_web', 'open_folder', 'changelog', 'update_state', 'update_check', 'update_download']) {
    await app.api(action, { id: 'sample', on: true, site: 'discord' });
  }
  assert.deepEqual(app.calls.filter(([name]) => name === 'service_action').map(([, value]) => value.action),
    ['mods_list', 'mods_folder', 'mods_toggle', 'mods_remove', 'open_web', 'open_folder', 'changelog', 'update_state', 'update_check', 'update_download']);
  const error = desktop({}, { serviceError: '서명이 일치하지 않습니다.' });
  assert.match((await error.api('update_install', {})).error, /서명/);
});

test('mobile selects and persists every macro feature before starting its native runner', async () => {
  const app = mobile();
  await app.ui.loadMacro();
  const features = ['mfish', 'mpop', 'mitem', 'mmerch', 'mcraft', 'mmatch', 'mblood'];
  for (const feature of features) await app.listeners.get('change')({ target: { disabled: false, type: 'checkbox', checked: true, dataset: { macro: feature + '.enabled' } } });
  await app.ui.changeMacro();
  assert.deepEqual(app.calls.find(([name]) => name === 'dua_start')[1].features, features);
  const saved = app.calls.filter(([name]) => name === 'save_macro_settings').at(-1)[1].config;
  assert.equal(saved.macro_on, false);
  for (const feature of features) assert.equal(saved[feature].enabled, true);
  const settings = app.ui.macroSettings();
  assert.match(settings, /자동 포션 제작/);
  assert.match(settings, /상인 자동 구매/);
  assert.match(settings, /자동 메모리 매치/);
  assert.match(settings, /position:base.ocr_region/);
  assert.doesNotMatch(settings, /준비 중|Python|data-action="unsupported"/);
});

test('mobile planned restart remains stoppable during a slow save without restoring the old generation', async () => {
  let release;
  const app = mobile({ configSaveWait: new Promise(resolve => { release = resolve; }) });
  await app.ui.changeMacro();
  const editing = app.ui.saveMacro(next => { next.mitem.enabled = true; });
  await flush();
  assert.equal(app.ui.current.dua.running, false);
  assert.equal(app.element('macro-toggle').textContent, '정지');
  await app.ui.changeMacro();
  release();
  await editing;
  assert.equal(app.ui.current.dua.running, false);
  assert.equal(app.calls.filter(([name]) => name === 'dua_start').length, 1);
});

test('mobile input setup, game tasks, destinations, position picks and updater reach their native handlers', async () => {
  const app = mobile();
  await app.ui.perform('inputSetup');
  assert.equal(app.calls.find(([name]) => name === 'android_setup')[1].action, 'inputSetup');
  await app.ui.perform('task:mitem_test');
  await app.ui.perform('go:craft');
  await app.ui.perform('mark:sell');
  await app.ui.perform('position:base.ocr_region');
  await app.ui.perform('update:check');
  const actions = app.calls.filter(([name]) => name === 'automation_action').map(([, value]) => value);
  assert.equal(actions[0].action, 'mitem_test');
  assert.equal(actions[1].action, 'go');
  assert.equal(actions[1].payload.destination, 'craft');
  assert.equal(actions[2].action, 'waypoint_mark');
  assert.equal(actions[2].payload.destination, 'sell');
  assert.equal(app.calls.find(([name]) => name === 'pick_macro_position')[1].region, true);
  assert.equal(app.calls.find(([name, value]) => name === 'service_action' && value.action === 'update_check')[1].action, 'update_check');
});


test('native PC state, stats and poll preserve original mutable SDK hooks and real counter clocks', async () => {
  const app = desktop({}, {
    mods: [{ id: 'sample', loaded: true }],
    metadata: { logs: [{ time: 10, msg: '실제 인벤토리 OCR 결과' }], stats: { crafted: 9, since: 4 } },
    hook(event, args) {
      if (event === 'state') args[0].sample_state = true;
      if (event === 'poll') args[0].sample_poll = true;
      return { args, results: event === 'stats' ? [{ sample_stat: 3, fish_success: 'invalid' }] : [] };
    },
  });
  const nativeState = await app.api('state', {});
  assert.equal(nativeState.error, undefined);
  assert.equal(nativeState.sample_state, true);
  app.status.stats.success = 2;
  app.status.automation = { running: true, active: { kind: 'items' }, clock: 100, itemNext: { strange: 135 }, nextMatch: 220, recoveryLeft: 6,
    used: { 'Strange Controller': 4, 'Cursed Rune Fragment': 1 }, crafted: 2 };
  const out = await app.api('poll', {});
  assert.equal(out.sample_poll, true);
  assert.equal(out.stats.session.sample_stat, 3);
  assert.equal(out.stats.session.fish_success, 2);
  assert.equal(out.stats.all.crafted, 9);
  assert.equal(out.mitem.items.strange.used, 4);
  assert.equal(out.mitem.items.strange.left, 35);
  assert.equal(out.mmatch.next_in, 120);
  assert.equal(out.crash.left, 6);
  assert.equal(out.mblood.used, 1);
  assert.equal(out.logs.filter(line => line.msg === '실제 인벤토리 OCR 결과').length, 1);
  assert.equal((await app.api('poll', {})).logs.filter(line => line.msg === '실제 인벤토리 OCR 결과').length, 0);
});


test('an automatic private-session recovery cannot be started twice and remains stoppable', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const elements = new Map();
  const element = id => { if (!elements.has(id)) elements.set(id, {}); return elements.get(id); };
  const context = { window: { acruxDesktopApi() {} }, lastSession: null, $: element, setText: (node, value) => { node.textContent = value; } };
  vm.runInNewContext(source.slice(source.indexOf('function updateSession('), source.indexOf('function applyDesktopText(')) + '\nglobalThis.renderSession = updateSession;', context);
  context.renderSession({ supported: true, state: 'recovering', message: '실제 서버 재연결 중' });
  assert.equal(element('desktopSessionStart').disabled, true);
  assert.equal(element('desktopSessionStop').disabled, false);
  assert.equal(element('desktopGameViewOpen').disabled, true);
  assert.equal(element('desktopSessionMessage').textContent, '실제 서버 재연결 중');
  context.renderSession({ supported: true, state: 'error', message: '재연결 실패' });
  assert.equal(element('desktopSessionStart').disabled, false);
});

function ocrControls(native, reply = { engine: 'Tesseract' }) {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const elements = new Map(), saves = [], timers = [], calls = [];
  const element = id => {
    if (!elements.has(id)) elements.set(id, { value: 'rapid', disabled: false,
      innerHTML: '<option value="auto">auto</option><option value="rapid">RapidOCR</option><option value="windows">Windows OCR</option>',
      listeners: {}, addEventListener(name, callback) { this.listeners[name] = callback; } });
    return elements.get(id);
  };
  const config = { ocr_engine: 'rapid' };
  const context = { window: native ? { acruxDesktopApi() {} } : {}, config, $: element,
    esc: value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;'),
    setText: (node, value) => { node.textContent = value; },
    api: async name => { calls.push(name); return reply; }, queueSave: patch => saves.push(patch),
    setTimeout: (callback, delay) => timers.push({ callback, delay }) };
  vm.runInNewContext(source.slice(source.indexOf('function configureNativeOcr('), source.indexOf('function fillAcrux('))
    + '\nglobalThis.ocrUi = { configureNativeOcr, refreshOcrInfo };', context);
  vm.runInNewContext(source.slice(source.indexOf("$('acOcr').addEventListener('change'"), source.indexOf("$('acLang').addEventListener('change'")), context);
  return { ...context.ocrUi, element, saves, timers, calls, config };
}

test('native OCR controls show each actual OS engine and cannot save an ignored legacy choice', async () => {
  for (const engine of ['Tesseract', 'Windows.Media.Ocr', 'Apple Vision']) {
    const ui = ocrControls(true, { engine });
    ui.configureNativeOcr();
    await ui.refreshOcrInfo();
    assert.deepEqual(ui.calls, ['ocr_info']);
    assert.equal(ui.element('acOcr').value, 'auto');
    assert.equal(ui.element('acOcr').disabled, true);
    assert.match(ui.element('acOcr').innerHTML, new RegExp(engine.replaceAll('.', '\\.')));
    assert.doesNotMatch(ui.element('acOcr').innerHTML, /value="rapid"|value="windows"/);
    assert.equal(ui.element('acOcrNow').textContent, engine);
    assert.match(ui.element('acOcrHelp').innerHTML, /Linux · Tesseract/);
    assert.match(ui.element('acOcrHelp').innerHTML, /Windows · Windows OCR/);
    assert.match(ui.element('acOcrHelp').innerHTML, /macOS · Apple Vision/);
    assert.doesNotMatch(ui.element('acOcrHelp').innerHTML, /RapidOCR/);
    ui.element('acOcr').listeners.change({ target: { value: 'windows' } });
    assert.equal(ui.config.ocr_engine, 'auto');
    assert.deepEqual(ui.saves, []);
    assert.deepEqual(ui.timers, []);
  }
  const failing = ocrControls(true, { error: 'OCR 정보를 읽지 못했습니다.' });
  failing.configureNativeOcr();
  await failing.refreshOcrInfo();
  assert.equal(failing.element('acOcrNow').textContent, 'OCR 정보를 읽지 못했습니다.');
});

test('Python OCR choices and saving retain their existing behavior', async () => {
  const ui = ocrControls(false, { engine: 'Windows OCR', rapid: false });
  const previous = ui.element('acOcr').innerHTML;
  ui.configureNativeOcr('Tesseract');
  assert.equal(ui.element('acOcr').innerHTML, previous);
  assert.equal(ui.element('acOcr').disabled, false);
  assert.equal(ui.config.ocr_engine, 'rapid');
  ui.element('acOcr').listeners.change({ target: { value: 'windows' } });
  assert.deepEqual({ ...ui.saves[0] }, { ocr_engine: 'windows' });
  assert.equal(ui.timers[0].delay, 600);
  await ui.refreshOcrInfo();
  assert.equal(ui.element('acOcrNow').textContent, 'Windows OCR (RapidOCR 없음)');
  const html = fs.readFileSync(path.join(__dirname, '../src/web/index.html'), 'utf8');
  assert.match(html, /<option value="rapid">RapidOCR<\/option>/);
  assert.match(html, /<option value="windows">윈도우 OCR<\/option>/);
});

test('native OCR explanations translate while keeping real engine names intact', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/i18n.js'), 'utf8');
  const body = { querySelectorAll: () => [], childNodes: [] };
  const context = { document: { body, documentElement: {}, createTreeWalker: () => ({ nextNode: () => false }) }, NodeFilter: { SHOW_TEXT: 4 } };
  vm.runInNewContext(source + '\nglobalThis.translations = I18N;', context);
  const messages = [
    '운영체제 자동 선택',
    '아이템 · 상점 · 제작 · 메모리 매치 화면의 글자를 이 기기에서 읽습니다.',
    '이 앱은 운영체제에 맞는 OCR 엔진을 자동으로 사용합니다.',
    '<b>Linux · Tesseract</b>: 영어 데이터와 앱 언어에 맞는 한국어·일본어 언어 데이터를 사용합니다.',
    '<b>Windows · Windows OCR</b>: Windows에 설치된 사용자 언어로 화면을 읽습니다.',
    '<b>macOS · Apple Vision</b>: macOS의 기본 문자 인식 기능으로 화면을 읽습니다.',
  ];
  for (const lang of ['en', 'ja']) {
    context.translations.set(lang);
    for (const message of messages) assert.doesNotMatch(context.translations.t(message), /[가-힣]/);
    assert.match(context.translations.t(messages[3]), /Tesseract/);
    assert.match(context.translations.t(messages[4]), /Windows OCR/);
    assert.match(context.translations.t(messages[5]), /Apple Vision/);
  }
});


test('mobile guides describe the current keyboard and panel calibration without requiring a test cast', async () => {
  const app = mobile();
  await app.ui.loadMacro();
  const readyGuide = app.ui.guideSteps.ready;
  assert.match(readyGuide.find(step => step.check === 'textInputReady').body, /Android 13 이상은 기존 키보드/);
  assert.match(readyGuide.find(step => step.check === 'textInputReady').body, /구형 Android.*때만/);
  const preparation = readyGuide.find(step => step.title === '로블록스 준비').body;
  assert.match(preparation, /실행할 기능/);
  assert.match(preparation, /카메라를 자동으로 올려 지형을 관측/);
  assert.match(preparation, /해안이나 저장한 장소까지 경로/);
  assert.match(preparation, /화면이 잠기면 정지합니다/);
  const calibration = app.ui.guideSteps.fish.find(step => step.check === 'calibrated').body;
  assert.match(calibration, /실제 버튼과 낚시 패널을 읽어 보정/);
  assert.doesNotMatch(calibration, /한 번 던져/);
  for (const lang of ['en', 'ja']) {
    app.ui.setLanguage(lang);
    assert.doesNotMatch(app.ui.macroSettings(), /Android 13 이상은 기존 키보드/);
    app.ui.startGuide('fish'); app.ui.moveGuide(1); app.ui.moveGuide(1);
    assert.doesNotMatch(app.element('guide').innerHTML, /실제 버튼과 낚시 패널/);
  }
});

test('finishing mobile preparation opens a fishing guide only when fishing is selected', async () => {
  for (const fishing of [false, true]) {
    const app = mobile();
    await app.ui.loadMacro();
    await app.ui.saveMacro(next => { next.mfish.enabled = fishing; next.mitem.enabled = !fishing; });
    app.ui.startGuide('ready');
    for (let i = 0; i < app.ui.guideSteps.ready.length; i++) app.ui.moveGuide(1, true);
    assert.equal(app.ui.guide?.id ?? null, fishing ? 'fish' : null);
  }
});

test('Windows foreground connection starts DUA without claiming background input', async () => {
  const app = desktop({ mfish: { enabled: true } }, { platform: 'windows' });
  app.status.dua.background = false;
  const started = await app.api('set_config', { patch: { macro_on: true } });
  assert.equal(started.error, undefined);
  assert.equal(started.config.macro_on, true);
  assert.equal((await app.api('poll', {})).dua.background, false);
});

test('optional service failure keeps the connection and controls available', async () => {
  const app = desktop({ mfish: { enabled: true } }, { serviceError: 'extension host offline' });
  const state = await app.api('state', {});
  assert.equal(state.version, '0.1.0');
  assert.equal(state.session.state, 'ready');
  assert.ok(state.error);
  const started = await app.api('set_config', { patch: { macro_on: true } });
  assert.equal(started.config.macro_on, true);
  const poll = await app.api('poll', {});
  assert.equal(poll.error, undefined);
  assert.equal(poll.session.state, 'ready');
});

test('join notice opens a disconnected game instead of submitting an impossible task', async () => {
  const app = desktop({}, { background: false, session: { state: 'stopped' } });
  const result = await app.api('ret_join', {});
  assert.equal(result.session.state, 'ready');
  assert.ok(app.calls.some(([name]) => name === 'desktop_session_start'));
  assert.ok(!app.calls.some(([name]) => name === 'automation_action'));
});

test('native session and task messages translate including retries and backend details', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/i18n.js'), 'utf8');
  const body = { querySelectorAll: () => [], childNodes: [] };
  const context = { document: { body, documentElement: {}, createTreeWalker: () => ({ nextNode: () => false }) }, NodeFilter: { SHOW_TEXT: 4 } };
  vm.runInNewContext(source + '\nglobalThis.translations = I18N;', context);
  const messages = [
    '[게임 열기]로 Roblox를 연결한 뒤 다시 시도하세요.',
    '백그라운드 게임 연결이 종료됐습니다.',
    '게임 화면에 연결됐습니다. 작업을 기다립니다.',
    '게임 복귀 실행 중',
    'Windows GPU 창 캡처를 시작하지 못했습니다: 0x80070057',
    '게임 화면 재시도 중 (2/5): 게임 창이 변경됐거나 최소화됐습니다.',
    '다른 창으로 전환되어 매크로를 중지했습니다. Roblox에서 다시 시작하세요.',
    '이동할 지면과 목적지를 확인합니다.',
    '게임 연결됨 · Windows에서는 Roblox를 앞에 두고 실행합니다.',
  ];
  for (const lang of ['en', 'ja']) {
    context.translations.set(lang);
    for (const message of messages) assert.doesNotMatch(context.translations.t(message), /[가-힣]/, message);
    assert.match(context.translations.t(messages.find(message => message.includes('재시도'))), /2\/5/);
  }
});

test('fishing numeric controls reject both bounds and restore the last valid value', () => {
  const source = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const inputs = [], saves = [];
  const tune = {
    set innerHTML(html) {
      for (const match of html.matchAll(/<input type="number" min="([^"]+)" max="([^"]+)" step="([^"]+)" data-mfish-tune="([^"]+)" value="([^"]+)"/g)) {
        const listeners = {};
        inputs.push({ min: match[1], max: match[2], dataset: { mfishTune: match[4] }, value: match[5], invalid: '',
          addEventListener: (kind, fn) => listeners[kind] = fn,
          setCustomValidity(value) { this.invalid = value; },
          get validity() { return { valid: !this.invalid }; },
          emit(kind) { listeners[kind](); },
        });
      }
    },
    insertAdjacentHTML() {}, querySelectorAll: () => inputs,
  };
  const config = { mfish: { bite_max: 60 } };
  const elements = { mfishTune: tune, mfishForm: {}, mfishDebug: { addEventListener() {} } };
  const context = { config, window: { acruxDesktopApi: true }, $: id => elements[id],
    queueSave: p => saves.push(structuredClone(p)), onRow: () => '', readyRow: () => '', syncReady() {}, lastMfish: null };
  vm.runInNewContext(source.slice(source.indexOf('const mfish ='), source.indexOf('function updateMfish')) + '\nrenderMfish();', context);
  const input = inputs.find(i => i.dataset.mfishTune === 'bite_max');
  assert.equal(input.max, '600');
  for (const bad of ['-5', '9999']) {
    input.value = bad; input.emit('input');
    assert.equal(config.mfish.bite_max, 60);
    assert.ok(input.invalid);
    input.emit('change');
    assert.equal(Number(input.value), 60);
  }
  assert.equal(saves.length, 0);
  input.value = '75'; input.emit('input'); input.emit('change');
  assert.equal(config.mfish.bite_max, 75);
  assert.equal(saves[0].mfish.bite_max, 75);
});
