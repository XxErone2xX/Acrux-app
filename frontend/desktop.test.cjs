const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const ts = require('typescript');
const defaults = JSON.parse(fs.readFileSync(path.join(__dirname, 'desktop-defaults.json'), 'utf8'));
const source = fs.readFileSync(path.join(__dirname, 'desktop.ts'), 'utf8');

function desktop(options = {}) {
  let config = structuredClone(defaults);
  const calls = [], errors = [], elements = [];
  const current = { generation: 0, phase: 'idle', version: '0.1.0', captureReady: true, inputReady: true,
    gameAvailable: false, calibrated: false, positions: null, message: '대기 중', measuringElapsed: null,
    macroRunning: false, settingsError: null,
    featureStatuses: Object.fromEntries(['mfish','mpop','mblood','mitem','mmerch','mcraft','mmatch'].map(name => [name, { running: false, msg: '대기', error: null, completedAt: 0 }])),
    measuredSeconds: null, stats: { success: 0, junk: 0, fail: 0, full: 0 }, totals: { success: 1, junk: 0, fail: 0, full: 0 },
    tuning: { biteSeconds: 60, leadMs: 0, targetPercent: 0, deadband: 0, clickMs: 20, clickGapMs: 45, castRetries: 3, adaptive: false, tapAbove: false } };
  class Element {
    constructor(tag) { this.tag = tag; this.children = []; this.listeners = {}; this.style = {}; elements.push(this); }
    setAttribute(key, value) { this[key] = value; }
    append(...children) { this.children.push(...children); }
    addEventListener(event, fn) { (this.listeners[event] ??= []).push(fn); }
    emit(event) { for (const fn of this.listeners[event] ?? []) fn({ preventDefault() {} }); }
    getContext() { return { putImageData: image => { this.image = image; } }; }
    showModal() { this.open = true; }
    close() { this.open = false; this.emit('close'); }
    remove() { this.removed = true; }
    focus() {}
  }
  const invoke = async (name, payload) => {
    calls.push([name, structuredClone(payload)]);
    if (name === 'macro_settings') { if (options.configError) throw new Error(options.configError); return structuredClone(config); }
    if (name === 'save_macro_settings') {
      if (options.saveWait) await options.saveWait;
      if (payload.config.mfish.bite_max > 600) throw new Error('낚시 설정의 범위를 확인하세요.');
      config = structuredClone(payload.config); ++current.generation; return structuredClone(config);
    }
    if (name === 'online_status') { if (options.onlineError) throw new Error(options.onlineError); return { share: true, online: 12 }; }
    if (name === 'desktop_snapshot') return options.snapshot;
    if (name === 'desktop_open_game') { current.gameAvailable = true; return structuredClone(current); }
    if (name === 'macro_action') {
      if (payload.action !== 'stop') assert.equal(payload.generation, current.generation);
      if (!['start','stop'].includes(payload.action)) throw new Error(`Unexpected macro action ${payload.action}`);
      ++current.generation;
      current.macroRunning = payload.action === 'start';
      current.phase = current.macroRunning ? 'searching' : 'idle';
      current.featureStatuses.mfish.running = current.macroRunning && config.mfish.enabled;
      return structuredClone(current);
    }
    if (name === 'desktop_navigation' || name === 'desktop_cancel_pick' || name === 'desktop_record' || name === 'desktop_move_place' || name === 'desktop_arrive' || name === 'desktop_route_clear') return;
    if (name === 'desktop_pick') return [.2,.3];
    if (name === 'desktop_pick_region') return [.1,.2,.7,.8];
    throw new Error(`Unexpected command ${name}`);
  };
  const modules = {
    '@tauri-apps/api/core': { invoke }, './desktop-defaults.json': defaults,
    './engine': {
      status: async () => { if (options.statusError) throw new Error(options.statusError); return structuredClone(current); },
      control: async (action, generation) => {
        calls.push(['control', { action, generation }]);
        if (action !== 'stop') assert.equal(generation, current.generation);
        ++current.generation; current.phase = action === 'start' ? 'searching' : 'idle'; return structuredClone(current);
      },
      macroAction: (action, payload = {}, generation) => invoke('macro_action', { action, payload, generation }),
    },
    './biome': { biomeStatus: async () => { if (options.biomeError) throw new Error(options.biomeError); return { settings: {}, history: [], current: null, owner: '', watching: false }; } },
  };
  const window = { dispatchEvent: event => errors.push(event.detail) };
  const storage = new Map();
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, esModuleInterop: true } }).outputText, {
    exports: {}, require: name => { assert.ok(name in modules, name); return modules[name]; },
    window, document: { createElement: tag => new Element(tag), body: new Element('body') },
    location: { href: 'http://localhost/desktop/index.html' }, URL, structuredClone, ArrayBuffer, DataView, Uint8ClampedArray,
    localStorage: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value) },
    CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
    ImageData: class { constructor(data, width, height) { Object.assign(this, { data, width, height }); } }, setTimeout,
  });
  return { api: window.acruxDesktopApi, calls, current, errors, elements, stored: () => structuredClone(config) };
}

const flush = async () => { for (let i=0;i<24;i++) await Promise.resolve(); };

test('state retains usable default configuration when native or optional services fail', async () => {
  const app = desktop({ configError: 'profile unavailable', statusError: 'state unavailable', biomeError: 'biome unavailable', onlineError: 'online unavailable' });
  const state = await app.api('state', {});
  assert.equal(state.config.lang, defaults.lang);
  assert.ok(state.config.mfish && Array.isArray(state.config.move.places));
  assert.equal(state.status[0], 'stopped');
  assert.equal(state.error, undefined, 'the original UI can still initialize its controls');
  assert.ok(app.errors.some(error => /state unavailable/.test(error)));
});

test('rejected tuning cannot poison subsequent independent native configuration saves', async () => {
  const app = desktop(); await app.api('state', {});
  const invalid = await app.api('set_config', { patch: { mfish: { ...defaults.mfish, bite_max: 9999 } } });
  assert.match(invalid.error, /범위/);
  assert.equal(invalid.config.mfish.bite_max, 60);
  const valid = await app.api('set_config', { patch: { mitem: { ...defaults.mitem, enabled: true } } });
  assert.equal(valid.error, undefined);
  assert.equal(app.stored().mfish.bite_max, 60);
  assert.equal(app.stored().mitem.enabled, true);
  assert.equal(app.calls.some(([name]) => name === 'save_tuning' || name === 'save_online' || name === 'save_biome'), false);
});

test('stop preempts a pending native save and its stale start cannot run', async () => {
  let resume; const gate = new Promise(resolve => { resume = resolve; });
  const app = desktop({ saveWait: gate }); await app.api('state', {});
  const pending = app.api('set_config', { patch: { mfish: { ...defaults.mfish, enabled: true }, macro_on: true } });
  await flush();
  const stopped = app.api('set_config', { patch: { macro_on: false } }); await flush();
  assert.ok(app.calls.some(([name,p]) => name === 'macro_action' && p.action === 'stop'));
  resume(); await Promise.all([pending, stopped]);
  assert.equal(app.calls.some(([name,p]) => name === 'macro_action' && p.action === 'start'), false);
  assert.equal(app.stored().macro_on, false);
});

test('native region and recording actions preserve the main movement API rather than timed route substitutes', async () => {
  const app = desktop(); await app.api('state', {});
  const region = await app.api('mpos_region', { feat: 'base', key: 'ocr_region' });
  assert.deepEqual(region.pos, [.1,.2,.7,.8]);
  assert.deepEqual(app.stored().base.ocr_region, [.1,.2,.7,.8]);
  await app.api('move_record', { place: 2 });
  await app.api('move_place', { place: 3 });
  assert.ok(app.calls.some(([name,p]) => name === 'desktop_record' && p.place === 2));
  assert.ok(app.calls.some(([name,p]) => name === 'desktop_move_place' && p.place === 3));
  assert.equal(app.calls.some(([name]) => name === 'desktop_waypoint'), false);
});

test('readonly snapshot uses actual RGBA and cannot send game input', async () => {
  const buffer = new ArrayBuffer(16), header = new DataView(buffer);
  header.setUint32(0,2,true); header.setUint32(4,1,true);
  new Uint8Array(buffer,8).set([1,2,3,255,4,5,6,255]);
  const app = desktop({ snapshot: buffer });
  const result = await app.api('game_view', {});
  assert.equal(result.ok, true);
  const canvas = app.elements.find(el => el.tag === 'canvas');
  assert.deepEqual([...canvas.image.data], [1,2,3,255,4,5,6,255]);
  assert.equal(Object.keys(canvas.listeners).length, 0);
  assert.deepEqual(app.calls.map(([name]) => name), ['desktop_snapshot']);
});

test('native save rejection restores failed form values while preserving a newer unrelated edit', async () => {
  const app = fs.readFileSync(path.join(__dirname, '../src/web/app.js'), 'utf8');
  const fragment = app.slice(app.indexOf('async function sendSave()'), app.indexOf('// ---------------------------------------------------------------- 로그'));
  const renders = [], context = {
    window: { acruxDesktopApi() {} }, config: { mfish: { bite_max: 9999 }, mitem: { enabled: true } },
    pending: { mfish: { bite_max: 9999 } }, saveTimer: null, LOCAL_KEYS: ['mfish','mitem'],
    api: async () => ({ config: { mfish: { bite_max: 60 }, mitem: { enabled: false } }, error: '범위 오류' }),
    toast() {}, renderSummary() {}, renderMpos() {}, rerenderMove() {}, setTimeout() {}, clearTimeout() {},
  };
  for (const name of ['fillPlay','fillBiome','fillMpop','fillRet','renderMfish','renderMitem','renderMmerch','renderMcraft','renderMmatch','renderMblood']) context[name] = () => renders.push(name);
  vm.runInNewContext(fragment + '\nglobalThis.run=sendSave;', context);
  await context.run();
  assert.equal(context.config.mfish.bite_max,60);
  assert.equal(context.config.mitem.enabled,true);
  assert.ok(renders.includes('renderMfish'));
});


test('new result calibration reaches the main UI without erasing it on a partial later observation', async () => {
  const app = desktop();
  app.current.positions = { panel: [.2,.2,.8,.8], reel: [.1,.2,.9,.8], fish: [.3,.7], bar: [.2,.3,.8,.4], result: [.2,.25,.8,.7], close: [.75,.3], title: [.5,.3] };
  const complete = await app.api('state', {});
  assert.deepEqual(complete.config.mfish.result_region, [.2,.25,.8,.7]);
  assert.deepEqual(complete.config.mfish.close_pos, [.75,.3]);
  assert.deepEqual(complete.config.mfish.title_pos, [.5,.3]);
  app.current.positions = { panel: [.2,.2,.8,.8], reel: [.1,.2,.9,.8], fish: [.3,.7], bar: [.2,.3,.8,.4], result: null };
  const partial = await app.api('state', {});
  assert.deepEqual(partial.config.mfish.result_region, complete.config.mfish.result_region);
  assert.deepEqual(partial.config.mfish.close_pos, complete.config.mfish.close_pos);
});
