import { dua } from "./dua";

type Bag = Record<string, unknown>;
export interface ModInfo {
  id: string; name: string; runtime: string; entry?: string;
  enabled: boolean; loaded: boolean; error: string; settings?: Bag;
  [key: string]: unknown;
}
export type ModField = { key: string; label: string; type: "text" | "number" | "checkbox"; min?: number; max?: number; step?: number }
  | { label: string; type: "button"; run: () => unknown | Promise<unknown> }
  | { label: string; type: "info" };
type Listener = (state?: unknown) => unknown | Promise<unknown>;
export interface ModContext {
  dua: typeof dua;
  settings: Bag;
  save: (patch: Bag) => Promise<void>;
  ui: (fields: ModField[]) => void;
  on: (event: "ready" | "poll" | "stop" | "shutdown", listener: Listener) => void;
  log: (message: string) => void;
}
interface Instance {
  settings: Bag; fields: ModField[]; listeners: Map<string, Listener[]>;
  loaded: boolean; error: string; busy: boolean; saving: Promise<void>; pendingStop?: boolean;
}
type Service = (action: string, payload?: Bag) => Promise<Bag>;
declare global { interface Window { AcruxPortableMods: { render: (target: HTMLElement) => void } } }

export function pickModPackage(service: Service): Promise<Bag> {
  return new Promise(resolve => {
    const input = document.createElement("input");
    input.type = "file"; input.accept = ".acxm,.acrux,.zip"; input.hidden = true;
    const finish = (result: Bag) => { input.remove(); resolve(result); };
    input.addEventListener("cancel", () => finish({ cancel: true }));
    input.addEventListener("change", async () => {
      const file = input.files?.[0];
      if (!file) return finish({ cancel: true });
      if (file.size > 30 * 1024 * 1024) return finish({ error: "모드 파일은 30MB 이하로 고르세요." });
      try { finish(await service("mods_install", { name: file.name, bytes: Array.from(new Uint8Array(await file.arrayBuffer())) })); }
      catch (error) { finish({ error: String(error) }); }
    });
    document.body.append(input); input.click();
  });
}

/** One mod runtime and one field renderer for all four device shells. */
export class PortableMods {
  private instances = new Map<string, Instance>();
  private started = false;
  private running = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private closing = false;
  constructor(private service: Service,
    private loadModule = (url: string): Promise<{ setup: (context: ModContext) => unknown }> => import(/* @vite-ignore */ url)) {
    window.addEventListener?.("pagehide", () => { void this.shutdown(); });
  }

  async start(items: ModInfo[]) {
    if (this.started) return;
    this.started = true;
    for (const item of items.filter(item => item.runtime === "javascript" && item.enabled)) {
      const instance: Instance = { settings: structuredClone(item.settings ?? {}), fields: [], listeners: new Map(),
        loaded: false, error: "", busy: false, saving: Promise.resolve() };
      this.instances.set(item.id, instance);
      try {
        if (!item.entry) throw new Error("모드 진입점이 없습니다.");
        const module = await this.loadModule(item.entry);
        if (typeof module.setup !== "function") throw new Error("모드 setup(ctx)가 없습니다.");
        await module.setup({ dua, settings: instance.settings,
          save: patch => this.save(instance, item.id, patch),
          ui: fields => { instance.fields = fields; },
          on: (event, listener) => { const list = instance.listeners.get(event) ?? []; list.push(listener); instance.listeners.set(event, list); },
          log: message => { console.info(`[mod:${item.id}]`, message); },
        });
        instance.loaded = true;
        await this.emit(instance, "ready");
      } catch (error) { instance.error = String(error); instance.loaded = false; }
    }
    if ([...this.instances.values()].some(instance => instance.loaded && (instance.listeners.has("poll") || instance.listeners.has("stop")))) void this.poll();
  }

  list(items: ModInfo[]) {
    return items.map(item => {
      if (item.runtime !== "javascript") return item;
      const instance = this.instances.get(item.id);
      return { ...item, loaded: !!instance?.loaded, error: instance?.error ?? item.error };
    });
  }

  private save(instance: Instance, id: string, patch: Bag) {
    const pending = instance.saving.then(async () => {
      const settings = { ...instance.settings, ...structuredClone(patch) };
      const result = await this.service("mods_settings", { id, settings });
      Object.assign(instance.settings, result.settings);
    });
    instance.saving = pending.catch(() => undefined);
    return pending;
  }

  private async emit(instance: Instance, event: string, state?: unknown) {
    for (const listener of instance.listeners.get(event) ?? []) {
      try { await listener(state); } catch (error) { instance.error = String(error); }
    }
  }

  tick(state: unknown, running: boolean) {
    const stopped = this.running && !running;
    this.running = running;
    for (const instance of this.instances.values()) {
      if (stopped) instance.pendingStop = true;
      if (!instance.loaded || instance.busy) continue;
      instance.busy = true;
      void (async () => {
        try {
          if (instance.pendingStop) { instance.pendingStop = false; await this.emit(instance, "stop", state); }
          await this.emit(instance, "poll", state);
        }
        finally { instance.busy = false; }
      })();
    }
  }

  private async poll() {
    try {
      const state = await dua.status();
      if (!this.closing) this.tick(state, state.dua.running);
    } catch (error) { console.warn("mod state", error); }
    finally { if (!this.closing) this.timer = setTimeout(() => { void this.poll(); }, 500); }
  }

  async shutdown() {
    this.closing = true;
    clearTimeout(this.timer);
    for (const instance of this.instances.values()) await this.emit(instance, "shutdown");
  }

  render(target: HTMLElement, translate = (s: string) => s) {
    target.replaceChildren();
    for (const [id, instance] of this.instances) {
      if (!instance.loaded || !instance.fields.length) continue;
      const section = document.createElement("section"); section.className = "card form";
      const heading = document.createElement("h3"); heading.textContent = id; section.append(heading);
      const error = document.createElement("p"); error.className = "muted"; error.textContent = instance.error;
      for (const field of instance.fields) {
        const row = document.createElement("label"); row.className = "row";
        const label = document.createElement("span"); label.textContent = translate(field.label); row.append(label);
        if (field.type === "info") { section.append(row); continue; }
        const input = document.createElement(field.type === "button" ? "button" : "input") as HTMLInputElement;
        input.type = field.type; input.className = field.type === "button" ? "btn button" : "";
        if (field.type === "button") input.textContent = translate(field.label);
        else {
          if (field.type === "checkbox") input.checked = instance.settings[field.key] === true;
          else input.value = String(instance.settings[field.key] ?? "");
          if (field.type === "number") {
            for (const key of ["min", "max", "step"] as const) if (field[key] !== undefined) input[key] = String(field[key]);
          }
        }
        input.addEventListener(field.type === "button" ? "click" : "change", async () => {
          input.disabled = true;
          try {
            if (field.type === "button") await field.run();
            else {
              if (!input.checkValidity()) { input.reportValidity(); return; }
              const value = field.type === "checkbox" ? input.checked : field.type === "number" ? Number(input.value) : input.value;
              await this.save(instance, id, { [field.key]: value });
            }
            instance.error = "";
          } catch (reason) { instance.error = String(reason); }
          finally {
            if (field.type !== "button") {
              if (field.type === "checkbox") input.checked = instance.settings[field.key] === true;
              else input.value = String(instance.settings[field.key] ?? "");
            }
            error.textContent = instance.error; input.disabled = false;
          }
        });
        row.append(input); section.append(row);
      }
      section.append(error); target.append(section);
    }
  }
}
