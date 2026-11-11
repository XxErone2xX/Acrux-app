const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const ts = require('typescript');

function desktop(saved, options = {}) {
  const preferencesKey = 'acrux.desktop.preferences';
  const storage = new Map([[preferencesKey, JSON.stringify(saved)]]);
  const errors = [], calls = [];
  const window = { dispatchEvent(event) { errors.push(event.detail); } };
  const stats = { success: 0, junk: 0, fail: 0, full: 0 };
  const status = {
    phase: 'idle', positions: null,
    tuning: { biteSeconds: 60, leadMs: 0, targetPercent: 0, clickGapMs: 45, castRetries: 3 }, stats, totals: stats,
    message: '대기 중', gameAvailable: false, version: '0.1.0',
    measuredSeconds: null, measuringElapsed: null,
    generation: 0,
    dua: { supported: true, running: false, route: null, reason: '대기 중', headlessTicks: 0, screenshotFrames: 0, transitions: 0 },
  };
  const biome = { settings: { enabled: false, player: '', psLink: '', webhooks: [], mentions: {}, roles: {} } };
  const modules = {
    './desktop-defaults.json': JSON.parse(fs.readFileSync(path.join(__dirname, 'desktop-defaults.json'), 'utf8')),
    './engine': {
      status: async () => structuredClone(status),
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
    window, document: { createElement: () => ({}), body: { append() {} } },
    location: { href: 'http://localhost/desktop/index.html' },
    localStorage: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) },
    structuredClone, URL, console,
    CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
  });
  return {
    api: window.acruxDesktopApi,
    stored: () => JSON.parse(storage.get(preferencesKey)),
    status, calls, errors,
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

test('main start configures navigation and starts DUA with the enabled feature list', async () => {
  const app = desktop({ mfish: { enabled: true } });
  const result = await app.api('set_config', { patch: { macro_on: true } });
  assert.equal(result.config.macro_on, true);
  assert.deepEqual(app.calls.map(([name]) => name), ['desktop_navigation', 'desktop_start']);
  assert.deepEqual(app.calls[1][1], { features: ['mfish'], generation: 0 });
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

test('stop cancels a start waiting in navigation setup or at the backend', async () => {
  for (const stop of ['mfish_stop', 'set_config']) {
    for (const hook of ['navigationWait', 'startWait']) {
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
      if (hook === 'navigationWait') assert.equal(app.calls.some(([name]) => name === 'desktop_start'), false);
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
  assert.equal(app.calls.filter(([name]) => name === 'desktop_navigation').length, 1);
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
