import ast
import contextlib
import importlib.util
from pathlib import Path
import re
import sys
import threading
import types
import unittest
from unittest.mock import patch

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('fishing_under_test', ROOT / 'src/fishing.py')
fishing = importlib.util.module_from_spec(spec)
with patch.dict(sys.modules, {'macro': types.SimpleNamespace(key_down_now=lambda key: False)}):
    spec.loader.exec_module(fishing)


class Finished(Exception):
    pass


class Clock:
    def __init__(self):
        self.now = 100.0

    def time(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds
        if self.now > 120:
            raise AssertionError('Fishing did not finish the simulated sequence')


def button(kind):
    image = np.zeros((20, 30, 3), dtype=np.int16)
    if kind == 'fish':
        image[5:10, 5:10] = (20, 100, 240)
    elif kind == 'exit':
        image[5:10, 5:10] = (240, 30, 30)
    return image


class FishingTests(unittest.TestCase):
    def loop(self, state, retry=3, until=108):
        clock, clicks = Clock(), []
        cfg = {'fish_btn': [0.5, 0.5], 'bar_region': [0.2, 0.2, 0.8, 0.3], 'cast_retry': retry}
        fisher = fishing.Fisher(lambda: cfg, lambda *args: None)
        fisher._rect = lambda stop: (0, 0, 600, 400)
        fisher._bar_seen = lambda *args: False
        fisher._grab_box = lambda *args: button(state(clock, clicks))
        fisher._click_ratio = lambda *args: clicks.append(clock.time())
        def wait(seconds, stop):
            clock.sleep(seconds)
            if clock.time() >= until:
                raise fishing.Stopped()
        fisher._wait = wait
        with patch.object(fishing, 'time', clock), self.assertRaises(fishing.Stopped):
            fisher._loop(None, threading.Event())
        return fisher, clicks

    def test_slow_cast_is_not_cancelled(self):
        fisher, clicks = self.loop(lambda clock, clicks: 'exit' if clicks and clock.time() - clicks[0] >= 2.5 else 'fish')
        self.assertEqual(len(clicks), 1)
        self.assertEqual(fisher.stats['full'], 0)

    def test_already_cast_loop_does_not_tap_exit(self):
        fisher, clicks = self.loop(lambda *args: 'exit')
        self.assertEqual(clicks, [])
        self.assertEqual(fisher.stats['full'], 0)

    def test_fresh_state_prevents_false_full(self):
        reads = 0
        def state(clock, clicks):
            nonlocal reads
            if clicks and clock.time() - clicks[0] > fishing.CAST_WAIT:
                reads += 1
                return 'fish' if reads == 1 else 'exit'
            return 'fish'
        fisher, clicks = self.loop(state, retry=1)
        self.assertEqual(len(clicks), 1)
        self.assertEqual(fisher.stats['full'], 0)

    def test_full_inventory_still_stops_after_three_casts(self):
        fisher, clicks = self.loop(lambda *args: 'fish', until=115)
        self.assertEqual(len(clicks), 3)
        self.assertEqual(fisher.stats['full'], 1)
        self.assertTrue(all(b - a >= fishing.CAST_WAIT for a, b in zip(clicks, clicks[1:])))

    def calibrate(self, initial='fish', delay=2.5):
        clock, clicks = Clock(), []
        # Load the actual Bridge entry point without importing the Windows UI or starting app threads.
        tree = ast.parse((ROOT / 'src/app.py').read_text(encoding='utf-8'))
        bridge = next(node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == 'Bridge')
        method = next(node for node in bridge.body if isinstance(node, ast.FunctionDef) and node.name == '_autocal_run')
        namespace = {'fishing': fishing, 'time': clock, '_AutocalStop': Finished}
        macro = types.SimpleNamespace(
            client_rect=lambda hwnd: (0, 0, 600, 400), focus=lambda *args, **kwargs: None,
            key_down_now=lambda key: False, to_screen=lambda x, y, rect: (x, y),
            click=lambda *args: clicks.append(clock.time()),
            ScreenGrabber=lambda: contextlib.nullcontext(None))
        namespace['macro'] = macro
        exec(compile(ast.Module(body=[method], type_ignores=[]), 'app.py', 'exec'), namespace)
        image = types.SimpleNamespace(rgb=bytes(600 * 400 * 3), width=600, height=400)
        mss = types.SimpleNamespace(mss=lambda: contextlib.nullcontext(types.SimpleNamespace(grab=lambda box: image)))
        def current():
            return 'exit' if initial == 'exit' or clicks and clock.time() - clicks[0] >= delay else initial
        def reel(*args):
            raise Finished('reeling reached')
        fisher = types.SimpleNamespace(
            _grab_box=lambda *args: button(current()),
            _bar_seen=lambda *args: current() == 'exit' and clock.time() >= 100.2,
            _diamond_moved=lambda *args: False, _reel=reel)
        obj = types.SimpleNamespace(
            lock=threading.Lock(), data={}, fisher=fisher, _autocal_stop=threading.Event(),
            _banner_progress=lambda *args: None)
        with patch.dict(sys.modules, {'mss': mss}), patch.object(fishing, 'find_frames', return_value=[(150, 200, 450, 300)]):
            expected = 'reeling reached' if initial in ('fish', 'exit') else 'Fish'
            with self.assertRaisesRegex(Finished, expected):
                namespace['_autocal_run'](obj, 1)
        return clicks

    def test_slow_cast_calibration_uses_same_wait(self):
        self.assertEqual(len(self.calibrate()), 1)

    def test_calibration_does_not_cancel_existing_cast(self):
        self.assertEqual(self.calibrate(initial='exit'), [])

    def test_calibration_does_not_tap_unknown_screen(self):
        self.assertEqual(self.calibrate(initial='unknown'), [])

    def test_mobile_cast_wait_matches_python(self):
        source = (ROOT / 'android/app/src/main/java/com/acrux/mobile/core/Fisher.java').read_text(encoding='utf-8')
        wait = re.search(r'double CAST_WAIT = ([0-9.]+);', source)
        self.assertEqual(float(wait[1]), fishing.CAST_WAIT)



class ReelTests(unittest.TestCase):
    CFG = dict(fishing.DEFAULTS)

    def test_clicks_before_leaving_zone(self):
        # 구간 (100~180) 안쪽 6% (바 폭 400 → 24px) 에 들어오면 누름 · 가운데보다 오른쪽은 안 누름
        self.assertTrue(fishing.reel_decision(120, (100, 180), 0, self.CFG, 400))
        self.assertFalse(fishing.reel_decision(130, (100, 180), 0, self.CFG, 400))
        self.assertFalse(fishing.reel_decision(150, (100, 180), -500, self.CFG, 400, lead=0.15))

    def test_reclicks_while_still_falling_below_zone(self):
        ctl = fishing.ReelControl(self.CFG)
        t, x, clicks = 0.0, 90.0, []
        for _ in range(40):                       # 눌러도 안 올라오고 계속 떨어짐
            t, x = t + 0.01, x - 1.0
            if ctl.step(t, x, (100, 180), 400, [(100, 180)])[0]:
                ctl.clicked(t)
                clicks.append(t)
        self.assertGreaterEqual(len(clicks), 5)   # 0.2초씩 기다리면 2번뿐


if __name__ == '__main__':
    unittest.main()
