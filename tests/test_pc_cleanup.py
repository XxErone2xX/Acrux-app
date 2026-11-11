import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

import app
import biome
import core
import mods
import popping


class DesktopCleanupTests(unittest.TestCase):
    def test_old_settings_are_removed_and_shared_inventory_is_preserved(self):
        old = {
            "snipe": {"enabled": True}, "prime": {"key": "retired"},
            "discord_connect": True, "guild_ids": ["123"],
            "pop": {
                "match_threshold": 50, "inventory_pos": [0.1, 0.2],
                "search_pos": [0.3, 0.4],
                "templates": {"SINGULARITY": {"items": [{"name": "old"}]}},
                "delays": {"after_join": 99, "search": 0.2},
            },
            "base": {"search_pos": [0.5, 0.6]},
            "ret": {"ps_link": "own", "start_wait": 9, "after_macro": False, "steps": [{"type": "text"}]},
            "mods": {"sniping": {"enabled": True}, "example": {"enabled": False}},
            "mod_settings": {"sniping": {"token": "retired"}, "example": {"volume": 1}},
        }
        cleaned = core.normalize(old)
        self.assertTrue(set(core.REMOVED_KEYS).isdisjoint(cleaned))
        self.assertEqual(cleaned["base"]["inventory_pos"], [0.1, 0.2])
        self.assertEqual(cleaned["base"]["search_pos"], [0.5, 0.6])
        self.assertEqual(set(cleaned["pop"]), {"match_threshold", "delays"})
        self.assertEqual(cleaned["pop"]["match_threshold"], 50)
        self.assertEqual(core.normalize(cleaned)["pop"]["match_threshold"], 50)
        self.assertEqual(cleaned["pop"]["delays"]["search"], 0.2)
        self.assertNotIn("after_join", cleaned["pop"]["delays"])
        self.assertEqual(cleaned["ret"], {"ps_link": "own", "start_wait": 9})
        self.assertEqual(cleaned["mods"], {"example": {"enabled": False}})
        self.assertEqual(cleaned["mod_settings"], {"example": {"volume": 1}})
        self.assertEqual(set(cleaned["mpop"]["templates"]), set(popping.POP_BIOMES))

    def test_removed_session_automation_cannot_be_started(self):
        self.assertFalse(hasattr(popping.Popper, "start"))
        self.assertFalse(hasattr(popping, "SessionBiome"))
        self.assertFalse(hasattr(biome.BiomeWatcher, "mute_next_session"))
        self.assertTrue(callable(popping.MyServerPopper.start))
        self.assertTrue(callable(popping.ItemUser.start))

    def test_biome_notifications_and_two_argument_callback_are_preserved(self):
        events, sent = [], []
        settings = {"player": "Player", "webhooks": ["https://discord.com/api/webhooks/123/test"]}
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "Roblox.log"
            log.write_text('Players.Player.PlayerGui:WaitForChild("TutorialCursor")\n'
                           '[BloxstrapRPC] {"largeImage":{"hoverText":"Glitched"}}\n')
            watcher = biome.BiomeWatcher(lambda: settings, lambda: True,
                                         on_change=lambda previous, found: events.append((previous, found)))
            with patch.object(core.Bus, "on_log", lambda *args: None), patch.object(biome, "latest_log", return_value=log), patch.object(biome, "send_all", side_effect=lambda hooks, payload: sent.append(payload)):
                watcher.tick()
                with log.open("a") as stream:
                    stream.write('[BloxstrapRPC] {"largeImage":{"hoverText":"Cyberspace"}}\n')
                watcher.tick()
            self.assertEqual(events, [(None, "GLITCHED"), ("GLITCHED", "CYBERSPACE")])
            self.assertEqual(watcher.state()["current"], "CYBERSPACE")
            self.assertNotIn("muted", watcher.state())
            self.assertEqual(len(sent), 3)  # Enabled, previous biome ended, next biome started.

    def test_public_biome_event_keeps_the_existing_three_argument_contract(self):
        events = []
        hooks = mods.Hooks(lambda *args: None)
        hooks.on("biome", lambda previous, found, reserved: events.append((previous, found, reserved)))
        bridge = SimpleNamespace(data={"macro_on": False}, hooks=hooks)
        app.Bridge._on_biome_change(bridge, "GLITCHED", "CYBERSPACE")
        self.assertEqual(events, [("GLITCHED", "CYBERSPACE", False)])

    def test_retired_mod_is_excluded_from_scanning_and_installation(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(mods, "MOD_DIR", Path(directory) / "mods"):
            for name in ("sniping", "example"):
                folder = mods.MOD_DIR / name
                folder.mkdir(parents=True)
                (folder / "mod.json").write_text(json.dumps({"id": name, "name": name, "type": "plugin"}))
            self.assertEqual([mod.id for mod in mods.scan()], ["example"])
            archive = Path(directory) / "old.acxm"
            with ZipFile(archive, "w") as package:
                package.writestr("mod.json", json.dumps({"id": "sniping"}))
            with self.assertRaises(ValueError):
                mods.install(archive)


if __name__ == "__main__":
    unittest.main()
