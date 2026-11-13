"""Actual desktop Python SDK setup/API/hooks over stdio; no game I/O."""
import json
import pathlib
import queue
import subprocess
import sys
import tempfile
import threading
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKER = ROOT / "src-tauri/resources/mod_worker.py"


class Worker:
    def __init__(self):
        self.process = subprocess.Popen([sys.executable, str(WORKER)], stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                        text=True, encoding="utf-8", bufsize=1)
        self.messages = queue.Queue()
        self.calls = []
        self.config = {}
        self.ticket = 0
        def read():
            for line in self.process.stdout:
                self.messages.put(json.loads(line))
            self.messages.put(None)
        threading.Thread(target=read, daemon=True).start()

    def command(self, op, payload=None):
        self.ticket += 1
        self.write({"id": self.ticket, "op": op, "payload": payload or {}})
        while True:
            reply = self.messages.get(timeout=5)
            if reply is None:
                raise RuntimeError("Worker exited before its reply")
            if reply["kind"] == "call":
                self.calls.append(reply)
                action, body = reply["action"], reply["payload"]
                if action == "save_config":
                    self.config = body["config"]
                    value = self.config
                elif action == "state":
                    value = {"config": self.config, "macro_on": False}
                elif action == "config":
                    value = self.config
                elif action == "automation":
                    if body["payload"]["method"] == "grab":
                        value = {"__bgra__": "AQIDBA==", "width": 1, "height": 1}
                    else:
                        value = {"ok": True, "method": body["payload"]["method"]}
                else:
                    value = None
                self.write({"kind": "response", "id": reply["id"], "value": value})
                continue
            if reply.get("error"):
                raise RuntimeError(reply["error"])
            if reply["id"] != self.ticket:
                raise RuntimeError("Wrong command ownership")
            return reply.get("value")

    def write(self, value):
        self.process.stdin.write(json.dumps(value) + "\n")
        self.process.stdin.flush()

    def close(self):
        try:
            if self.process.poll() is None:
                self.command("shutdown")
                self.process.wait(timeout=3)
        finally:
            if self.process.poll() is None:
                self.process.kill()
                self.process.wait(timeout=3)
            self.process.stdin.close()
            self.process.stdout.close()


class ModWorkerTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.data = pathlib.Path(self.tmp.name)
        self.worker = Worker()

    def tearDown(self):
        self.worker.close()
        self.tmp.cleanup()

    def package(self, mid, source):
        folder = self.data / "mods" / mid
        folder.mkdir(parents=True)
        (folder / "mod.json").write_text(json.dumps({"id": mid, "name": mid, "version": "1.2.3", "type": "plugin"}))
        (folder / "main.py").write_text(source)

    def load(self):
        return self.worker.command("load", {"data": str(self.data), "sdk": str(ROOT / "src/mods.py"),
            "defaults": str(ROOT / "frontend/desktop-defaults.json"), "config": {}, "reserved": ["poll", "state", "set_config"]})

    def test_original_context_executes_real_setup_api_hooks_and_config_round_trip(self):
        self.package("sample", '''import macro\ndef setup(ctx):\n    ctx.config_keys({"sample_option": 3})\n    ctx.log("sample setup", "g")\n    @ctx.api("sample_call")\n    def call(payload):\n        ctx.settings["count"] = ctx.settings.get("count", 0) + payload["add"]\n        ctx.save()\n        return {"count": ctx.settings["count"], "native": macro.tap(0.2, 0.3, 40)}\n    def ready():\n        ctx.settings["ready"] = True\n        ctx.save()\n    ctx.on("ready", ready)\n    ctx.on("poll", lambda out, since: out.update(sample_seen=ctx.settings.get("count", 0)))\n''')
        loaded = self.load()
        self.assertTrue(loaded["mods"][0]["loaded"])
        self.assertEqual(loaded["apis"], ["sample_call"])
        self.assertEqual(loaded["config"]["sample_option"], 3)
        self.assertTrue(self.worker.config["mod_settings"]["sample"]["ready"])
        result = self.worker.command("call", {"name": "sample_call", "payload": {"add": 2}})
        self.assertEqual(result, {"count": 2, "native": {"ok": True, "method": "tap"}})
        self.assertEqual(self.worker.config["mod_settings"]["sample"]["count"], 2)
        native = [call for call in self.worker.calls if call["action"] == "automation"]
        self.assertEqual(native[0]["payload"], {"action": "sdk_call", "payload": {
            "module": "macro", "method": "tap", "args": [0.2, 0.3, 40], "kwargs": {}}})
        event = self.worker.command("event", {"event": "poll", "args": [{"existing": 1}, 0]})
        self.assertEqual(event["args"][0], {"existing": 1, "sample_seen": 2})

    def test_config_changes_and_stop_state_follow_native_events(self):
        self.package("settings", '''import core
def setup(ctx):
    ctx.on("start", lambda: ctx.settings.update(stopped=core.STOP.is_set(), value=ctx.bridge.cfg.get("native_value")))
    ctx.on("stop", lambda: ctx.settings.update(stopped=core.STOP.is_set()))
    ctx.on("macro", lambda on: ctx.settings.update(stopped=core.STOP.is_set()))
    @ctx.api("settings_value")
    def value(payload):
        if payload.get("save"):
            return ctx.bridge.api_set_config({"patch": {"native_value": payload["save"]}})
        return {"stopped": ctx.settings.get("stopped"), "value": ctx.settings.get("value"), "state": ctx.bridge.api_state()}
''')
        self.assertTrue(self.load()["mods"][0]["loaded"])
        self.worker.command("event", {"event": "start", "args": [], "config": {"native_value": 8}})
        started = self.worker.command("call", {"name": "settings_value", "payload": {}})
        self.assertEqual(started["value"], 8)
        self.assertFalse(started["stopped"])
        self.worker.command("event", {"event": "stop", "args": []})
        self.assertTrue(self.worker.command("call", {"name": "settings_value", "payload": {}})["stopped"])
        self.worker.command("event", {"event": "macro", "args": [True]})
        self.assertFalse(self.worker.command("call", {"name": "settings_value", "payload": {}})["stopped"])
        self.worker.command("event", {"event": "macro", "args": [False]})
        self.assertTrue(self.worker.command("call", {"name": "settings_value", "payload": {}})["stopped"])
        result = self.worker.command("call", {"name": "settings_value", "payload": {"save": 9}})
        self.assertEqual(result["config"]["native_value"], 9)

    def test_capture_adapter_restores_original_bgra_tuple(self):
        self.package("capture", '''import core
def setup(ctx):
    @ctx.api("sample_capture")
    def call(payload):
        pixels, width, height = core.grab([0, 0, 1, 1])
        return {"bytes": list(pixels), "width": width, "height": height}
''')
        self.assertTrue(self.load()["mods"][0]["loaded"])
        result = self.worker.command("call", {"name": "sample_capture", "payload": {}})
        self.assertEqual(result, {"bytes": [1, 2, 3, 4], "width": 1, "height": 1})

    def test_shutdown_runs_original_cleanup_hooks_and_flushes_settings_before_exit(self):
        self.package("cleanup", '''def setup(ctx):
    ctx.on("stop", lambda: ctx.settings.update(stopped=True))
    def shutdown():
        ctx.settings["closed"] = True
        ctx.save()
    ctx.on("shutdown", shutdown)
''')
        self.assertTrue(self.load()["mods"][0]["loaded"])
        self.assertEqual(self.worker.command("shutdown"), {"ok": True})
        self.worker.process.wait(timeout=3)
        self.assertEqual(self.worker.config["mod_settings"]["cleanup"], {"stopped": True, "closed": True})

    def test_reserved_api_collision_and_failed_setup_are_reported_without_disabling_other_mods(self):
        self.package("bad_api", 'def setup(ctx):\n    ctx.api("poll")(lambda payload: {})\n')
        self.package("bad_setup", 'def setup(ctx):\n    raise RuntimeError("setup failed")\n')
        self.package("good_mod", 'def setup(ctx):\n    ctx.on("active", lambda: True)\n')
        mods = {item["id"]: item for item in self.load()["mods"]}
        self.assertFalse(mods["bad_api"]["loaded"])
        self.assertIn("ValueError", mods["bad_api"]["error"])
        self.assertIn("setup failed", mods["bad_setup"]["error"])
        self.assertTrue(mods["good_mod"]["loaded"])
        self.assertEqual(self.worker.command("event", {"event": "active", "args": []})["results"], [True])

    def test_unregistered_api_never_invokes_native_and_retired_mod_is_not_loaded(self):
        self.package("sniping", 'def setup(ctx):\n    raise RuntimeError("must not execute")\n')
        self.assertEqual(self.load()["mods"], [])
        with self.assertRaisesRegex(RuntimeError, "Unknown mod API"):
            self.worker.command("call", {"name": "poll", "payload": {}})
        self.assertFalse(any(call["action"] == "automation" for call in self.worker.calls))


if __name__ == "__main__":
    unittest.main()
