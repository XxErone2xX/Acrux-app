"""Desktop Python mod SDK worker. Game I/O is always delegated to native IPC."""
import base64
import importlib.util
import json
import pathlib
import queue
import re
import sys
import threading
import types
import urllib.parse

_protocol = sys.stdout
sys.stdout = sys.stderr
_write_lock = threading.Lock()
_pending_lock = threading.Lock()
_pending = {}
_commands = queue.Queue()
_sequence = 0
_bridge = None
_mods = None


def send(value):
    line = json.dumps(value, ensure_ascii=False, default=lambda v: str(v), allow_nan=False)
    with _write_lock:
        _protocol.write(line + "\n")
        _protocol.flush()


def receive():
    for line in sys.stdin:
        try:
            value = json.loads(line)
            if value.get("kind") == "response":
                with _pending_lock:
                    target = _pending.get(value.get("id"))
                if target:
                    target.put(value)
            else:
                _commands.put(value)
        except (ValueError, TypeError):
            continue
    with _pending_lock:
        for target in _pending.values():
            try:
                target.put_nowait({"error": "Native host disconnected"})
            except queue.Full:
                pass
    _commands.put({"op": "shutdown", "id": 0})


def rpc(action, payload=None):
    global _sequence
    with _pending_lock:
        _sequence += 1
        ticket = _sequence
        result = _pending[ticket] = queue.Queue(maxsize=1)
    try:
        send({"kind": "call", "id": ticket, "action": action, "payload": payload or {}})
        reply = result.get(timeout=30)
        if reply.get("error"):
            raise RuntimeError(reply["error"])
        return reply.get("value")
    finally:
        with _pending_lock:
            _pending.pop(ticket, None)


class NativeModule(types.ModuleType):
    def __getattr__(self, name):
        if name.startswith("_"):
            raise AttributeError(name)
        def call(*args, **kwargs):
            value = rpc("automation", {"action": "sdk_call", "payload": {
                "module": self.__name__, "method": name, "args": args, "kwargs": kwargs}})
            if name == "grab" and isinstance(value, dict) and "__bgra__" in value:
                pixels = base64.b64decode(value["__bgra__"], validate=True)
                width, height = value["width"], value["height"]
                if not isinstance(width, int) or not isinstance(height, int) or width <= 0 or height <= 0 or len(pixels) != width * height * 4:
                    raise ValueError("Native capture dimensions do not match its BGRA pixels")
                return pixels, width, height
            return value
        return call


class Config:
    def __init__(self, data=None):
        self._data = dict(data or {})
        self._lock = threading.RLock()

    def set_data(self, data):
        with self._lock:
            self._data = dict(data)

    def get(self, key, default=None):
        with self._lock:
            return self._data.get(key, default)

    def __getattr__(self, name):
        return self.get(name)


def native_core(data_dir, defaults):
    core = types.ModuleType("core")
    core.DATA_BASE = pathlib.Path(data_dir)
    core.BASE = core.DATA_BASE
    core.CONFIG_PATH = core.DATA_BASE / "mod-config.json"
    core.DEFAULT_CONFIG = defaults
    core.NORMALIZERS = []
    core.STOP = threading.Event()
    core.Config = Config
    core.log = lambda message, color="": rpc("log", {"message": str(message), "color": color})
    core.status = lambda state, text="": rpc("status", {"state": state, "text": text})
    core.Bus = types.SimpleNamespace(on_log=core.log, on_status=core.status)

    def normalize(value):
        out = json.loads(json.dumps({**defaults, **dict(value or {})}))
        for normalizer in core.NORMALIZERS:
            out = normalizer(out)
        return out

    def save_config(value):
        rpc("save_config", {"config": value})
        if _bridge is not None:
            _bridge.data = dict(value)
            _bridge.cfg.set_data(value)

    def parse_link(url):
        for text in dict.fromkeys([str(url or ""), urllib.parse.unquote(str(url or ""))]):
            place = re.search(r"(?:/games/|[?&/]placeId=)(\d{1,20})(?!\d)", text, re.I)
            code = re.search(r"[?&](?:privateServerLink(?:Code)?|linkCode)=([A-Za-z0-9_-]{1,200})", text, re.I)
            if place and code:
                return "private", place[1], code[1]
            share = re.search(r"[?&]code=([A-Za-z0-9_-]{1,200})", text, re.I)
            if share and re.search(r"share[-_]?links?|/share\?|type=Server", text, re.I):
                return "share", share[1]
        return None

    def deeplink(url):
        value = parse_link(url)
        if not value:
            return None
        return (f"roblox://placeId={value[1]}&linkCode={value[2]}" if value[0] == "private"
                else f"roblox://navigation/share_links?code={value[1]}&type=Server")

    def weblink(url):
        value = parse_link(url)
        if not value:
            return None
        return (f"https://www.roblox.com/games/{value[1]}?privateServerLinkCode={value[2]}" if value[0] == "private"
                else f"https://www.roblox.com/share?code={value[1]}&type=Server")

    core.normalize = normalize
    core.load_config = lambda: rpc("config")
    core.save_config = save_config
    core.parse_link = parse_link
    core.to_deeplink = deeplink
    core.to_weblink = weblink
    core.open_link = lambda url: rpc("automation", {"action": "sdk_call", "payload": {
        "module": "core", "method": "open_link", "args": [url], "kwargs": {}}})
    core.__getattr__ = lambda name: getattr(NativeModule("core"), name)
    return core


class Bridge:
    def __init__(self, config, reserved):
        self.data = config
        self.cfg = Config(config)
        self.lock = threading.RLock()
        self.mod_apis = {}
        self.mod_keys = set(config)
        self.mods = []
        self.hooks = _mods.Hooks(self._on_log)
        self.reserved = set(reserved)

    def _on_log(self, message, color=""):
        rpc("log", {"message": str(message), "color": color})

    def _on_status(self, state, text=""):
        rpc("status", {"state": state, "text": text})

    def _save(self):
        rpc("save_config", {"config": self.data})
        self.cfg.set_data(self.data)

    def __getattr__(self, name):
        if name.startswith("api_"):
            action = name[4:]
            if action not in self.reserved:
                raise AttributeError(name)
            if action in {"state", "poll"}:
                return lambda payload=None: rpc("state")
            if action == "set_config":
                def set_config(payload=None):
                    current = rpc("config")
                    current.update((payload or {}).get("patch") or {})
                    value = rpc("save_config", {"config": current})
                    self.data = dict(value)
                    self.cfg.set_data(value)
                    return {"config": value}
                return set_config
            return lambda payload=None: rpc("automation", {"action": action, "payload": payload or {}})
        if name in {"fisher", "mover", "play", "ret", "mpop", "mitem", "mmerch", "mcraft", "mmatch", "mblood"}:
            return NativeModule(name)
        raise AttributeError(name)


def load(payload):
    global _bridge, _mods
    data = pathlib.Path(payload["data"])
    defaults = json.loads(pathlib.Path(payload["defaults"]).read_text("utf-8"))
    sys.modules["core"] = native_core(data, defaults)
    # These names intentionally resolve to native adapters, never legacy
    # foreground capture/SendInput or a second Python macro engine.
    for name in ("macro", "fishing", "popping", "rejoin", "move", "sell", "steps", "crafter", "merchant", "memmatch", "biome"):
        sys.modules[name] = NativeModule(name)
    spec = importlib.util.spec_from_file_location("mods", payload["sdk"])
    _mods = importlib.util.module_from_spec(spec)
    sys.modules["mods"] = _mods
    spec.loader.exec_module(_mods)
    _bridge = Bridge(dict(payload.get("config") or {}), payload.get("reserved", []))
    _mods.load_all(_bridge)
    _bridge.hooks.emit("ready")
    return {"mods": _mods.listing(_bridge), "apis": sorted(_bridge.mod_apis), "config": _bridge.data}


def command(value):
    op = value.get("op")
    payload = value.get("payload") or {}
    if op == "load":
        return load(payload)
    if _bridge is None:
        raise RuntimeError("Mod SDK is not initialized")
    if op == "list":
        return {"mods": _mods.listing(_bridge), "apis": sorted(_bridge.mod_apis)}
    if op == "call":
        name = str(payload.get("name") or "")
        if name not in _bridge.mod_apis:
            raise ValueError("Unknown mod API")
        return getattr(_bridge, "api_" + name)(payload.get("payload") or {})
    if op == "event":
        event = str(payload.get("event") or "")
        if isinstance(payload.get("config"), dict):
            _bridge.data = dict(payload["config"])
            _bridge.cfg.set_data(_bridge.data)
        args = payload.get("args") or []
        if event == "start" or event == "macro" and args and args[0] is True:
            sys.modules["core"].STOP.clear()
        elif event == "stop" or event == "macro" and args and args[0] is False:
            sys.modules["core"].STOP.set()
        results = _bridge.hooks.emit(event, *args)
        return {"results": results, "args": args, "config": _bridge.data}
    if op == "shutdown":
        sys.modules["core"].STOP.set()
        _bridge.hooks.emit("stop")
        _bridge.hooks.emit("shutdown")
        return {"ok": True}
    raise ValueError("Unknown mod worker command")


def main():
    threading.Thread(target=receive, name="mod-ipc-reader", daemon=True).start()
    while True:
        value = _commands.get()
        try:
            result = command(value)
            send({"kind": "result", "id": value.get("id"), "value": result})
        except Exception as error:
            send({"kind": "result", "id": value.get("id"), "error": f"{type(error).__name__}: {error}"})
        if value.get("op") == "shutdown":
            return


if __name__ == "__main__":
    main()
