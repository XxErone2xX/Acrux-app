const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const ts = require('typescript');

function desktop(saved) {
  const preferencesKey = 'acrux.desktop.preferences';
  const storage = new Map([[preferencesKey, JSON.stringify(saved)]]);
  const window = { dispatchEvent() {} };
  const stats = { success: 0, junk: 0, fail: 0, full: 0 };
  const status = {
    phase: 'idle', positions: null, tuning: {}, stats, totals: stats,
    message: '대기 중', gameAvailable: false, version: '0.1.0',
  };
  const biome = { settings: { enabled: false, player: '', psLink: '', webhooks: [], mentions: {}, roles: {} } };
  const modules = {
    './desktop-defaults.json': JSON.parse(fs.readFileSync(path.join(__dirname, 'desktop-defaults.json'), 'utf8')),
    './engine': { status: async () => structuredClone(status), control: async () => structuredClone(status) },
    './biome': { biomeStatus: async () => structuredClone(biome), saveBiome: async settings => ({ settings }) },
    '@tauri-apps/api/core': { invoke: async command => {
      if (command === 'online_status') return { share: true, online: 1 };
      if (command === 'save_online') return;
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
  for (const filename of ['app.js', 'index.html', 'style.css', 'i18n.js']) {
    const source = fs.readFileSync(path.join(__dirname, '../src/web', filename), 'utf8');
    assert.doesNotMatch(source, /스나이|スナイ|Prime Snipe|data-mpos-link|setPopLink|SNIPE_BIOMES|STAY_BIOMES|모든 채널 감시|디버그 포트/);
    if (filename.endsWith('.js')) new vm.Script(source, { filename });
  }
});
