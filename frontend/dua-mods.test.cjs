const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const test = require('node:test');
const ts = require('typescript');

class Element {
  constructor(tag) { this.tag = tag; this.children = []; this.listeners = {}; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
  addEventListener(name, fn) { this.listeners[name] = fn; }
  checkValidity() { return true; }
  reportValidity() {}
}
async function flush() { for (let i = 0; i < 30; ++i) await Promise.resolve(); }
function load(platform) {
  const calls = [], writes = [], timers = new Map();
  let id = 0;
  const state = { generation: 7, platform, dua: { running: false } };
  const invoke = async (name, payload) => {
    calls.push([name, payload]);
    if (name === 'status') return state;
    assert.equal(name, 'automation_action');
    assert.equal(payload.generation, 7);
    assert.equal(payload.action, 'sdk_call');
    assert.equal(payload.payload.module, 'dua');
    if (payload.payload.method === 'status') return state;
    if (payload.payload.method === 'observe') return { image: { __bgra__: 'AQIDBA==', width: 1, height: 1, origin: [4, 5] }, words: [['Fish', .5, .5, .2, .1]], capturedAt: 12, inputDeadline: 12.3, generation: 7, engine: 'local' };
    return { ok: true };
  };
  const context = { exports: {}, window: {}, console, structuredClone, Uint8Array, atob,
    document: { createElement: tag => new Element(tag) },
    setTimeout: fn => { timers.set(++id, fn); return id; }, clearTimeout: id => timers.delete(id) };
  const compile = file => ts.transpileModule(fs.readFileSync(path.join(__dirname, file), 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  context.require = name => { assert.equal(name, '@tauri-apps/api/core'); return { invoke }; };
  vm.runInNewContext(compile('dua.ts'), context);
  const { dua } = context.exports;
  context.exports = {};
  context.require = name => { assert.equal(name, './dua'); return { dua }; };
  vm.runInNewContext(compile('portable-mods.ts'), context);
  const service = async (action, payload) => { writes.push([action, structuredClone(payload)]); return { settings: structuredClone(payload.settings) }; };
  return { dua, ...context.exports, calls, writes, timers, state, service };
}

test('one DUA contract sends the same commands and same-frame observations on all four OSes', async () => {
  const transcripts = [];
  for (const platform of ['windows', 'linux', 'macos', 'android']) {
    const app = load(platform);
    await app.dua.capabilities();
    await app.dua.start(['mfish']);
    await app.dua.go('sell');
    await app.dua.mark('sell');
    await app.dua.gesture({ kind: 'stick', from: [.5, .5], to: [.6, .5], ms: 100 });
    const observation = await app.dua.observe([.1, .2, .8, .9]);
    assert.deepEqual([...observation.image.pixels], [1, 2, 3, 4]);
    assert.deepEqual(observation.image.origin, [4, 5]);
    assert.equal(observation.capturedAt, 12);
    await app.dua.stop();
    transcripts.push(structuredClone(app.calls.filter(([name]) => name === 'automation_action').map(([, value]) => value.payload)));
  }
  for (const transcript of transcripts) assert.deepEqual(transcript, transcripts[0]);
});

test('the same actual JavaScript mod exposes its fields, calls DUA and persists scoped settings on every OS', async () => {
  const module = await import('data:text/javascript,' + encodeURIComponent(`
    export async function setup(ctx) {
      Object.assign(ctx.settings, {enabled: false, count: 1, ...ctx.settings});
      ctx.ui([{type:'checkbox', key:'enabled', label:'Enabled'}, {type:'number', key:'count', label:'Count', min:1, max:10},
        {type:'button', label:'Move', run:()=>ctx.dua.go('coast')}]);
      await Promise.all([ctx.save({enabled:true}), ctx.save({count:3})]);
    }
  `));
  for (const platform of ['windows', 'linux', 'macos', 'android']) {
    const app = load(platform);
    const item = { id: 'sample', name: 'Sample', runtime: 'javascript', entry: 'native/sample/main.js', settings: {}, enabled: true, error: '' };
    const runtime = new app.PortableMods(app.service, async () => module);
    await runtime.start([item]);
    assert.equal(runtime.list([item])[0].loaded, true);
    assert.deepEqual(app.writes.at(-1), ['mods_settings', { id: 'sample', settings: { enabled: true, count: 3 } }]);
    const target = new Element('div'); runtime.render(target);
    const rows = target.children[0].children.filter(child => child.tag === 'label');
    assert.deepEqual(rows.map(row => row.children[0].textContent), ['Enabled', 'Count', 'Move']);
    await rows[2].children[1].listeners.click();
    assert.equal(app.calls.at(-1)[1].payload.method, 'go');
    assert.deepEqual([...app.calls.at(-1)[1].payload.args], ['coast']);
    await runtime.shutdown();
  }
});

test('one failed mod cannot claim loaded status or prevent another mod from starting', async () => {
  const app = load('android');
  const runtime = new app.PortableMods(app.service, async url => {
    if (url === 'bad') throw new Error('broken package');
    return { setup: ctx => ctx.ui([{type:'info',label:'Ready'}]) };
  });
  const items = ['bad', 'good'].map(id => ({ id, name: id, runtime:'javascript', entry:id, enabled:true, error:'' }));
  await runtime.start(items);
  assert.equal(runtime.list(items)[0].loaded, false);
  assert.match(runtime.list(items)[0].error, /broken package/);
  assert.equal(runtime.list(items)[1].loaded, true);
  await runtime.shutdown();
});

test('the shipped DUA example uses the same foundation without OS-specific fields or handlers', async () => {
  const module = await import('../examples/dua-coast-helper/main.js');
  const labels = [];
  for (const platform of ['windows', 'linux', 'macos', 'android']) {
    const app = load(platform);
    const item = { id:'dua_coast_helper', name:'Example', runtime:'javascript', entry:'main.js', enabled:true, error:'' };
    const runtime = new app.PortableMods(app.service, async () => module);
    await runtime.start([item]);
    const target = new Element('div'); runtime.render(target);
    const rows = target.children[0].children.filter(child => child.tag === 'label');
    labels.push(rows.map(row => row.children[0].textContent));
    await rows[0].children[1].listeners.click();
    await rows[4].children[1].listeners.click();
    assert.deepEqual(app.calls.filter(([name]) => name === 'automation_action').map(([, call]) => call.payload.method), ['go', 'stop']);
    await runtime.shutdown();
  }
  for (const value of labels) assert.deepEqual(value, labels[0]);
});

test('mod polling stays independent of UI visibility, never overlaps and retains stop during a slow handler', async () => {
  const app = load('android');
  let entered = 0, stopped = 0, release;
  const gate = new Promise(resolve => { release = resolve; });
  const runtime = new app.PortableMods(app.service, async () => ({ setup(ctx) {
    ctx.on('poll', async () => { ++entered; await gate; });
    ctx.on('stop', () => { ++stopped; });
  } }));
  await runtime.start([{id:'sample', name:'Sample', runtime:'javascript', entry:'main.js', enabled:true, error:''}]);
  await flush();
  assert.equal(entered, 1);
  runtime.tick({}, true); runtime.tick({}, false);
  assert.equal(entered, 1);
  release(); await flush(); runtime.tick({}, false); await flush();
  assert.equal(stopped, 1);
  assert.equal(entered, 2);
  assert.equal(app.calls.some(([, call]) => call?.payload?.method === 'observe'), false);
  await runtime.shutdown();
  assert.equal(app.timers.size, 0);
});
