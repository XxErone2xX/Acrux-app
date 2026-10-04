# -*- coding: utf-8 -*-
"""
Acrux snipe & popping macro — 창 화면은 HTML/CSS 로 그리고, 윈도우 기본 Edge 엔진(앱 모드)으로 띄운다.
감지 엔진은 watcher_core.py 그대로.

구조
  - 127.0.0.1 에만 열리는 작은 웹 서버가 화면 파일(web/)과 API(/api/...)를 제공
  - API 는 실행할 때마다 새로 만드는 비밀 토큰이 있어야 호출됨 (다른 프로그램/웹페이지 차단)
  - Edge 를 --app 모드 + 전용 프로필로 띄우고, 창이 닫히면 프로그램도 종료
"""
import json
import os
import re
import secrets
import subprocess
import sys
import threading
import time
import traceback
from collections import deque
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import watcher_core as core
import macro
import biome
import rejoin
import popping
import steps as stepmod
from version import VERSION

APP_NAME = "AcruxMacro"
if getattr(sys, "frozen", False):
    WEB_DIR = Path(getattr(sys, "_MEIPASS", core.BASE)) / "web"
else:
    WEB_DIR = Path(__file__).resolve().parent / "web"
DATA_DIR = Path(os.environ.get("LOCALAPPDATA", str(core.BASE))) / APP_NAME
LOG_FILE = core.DATA_BASE / "app.log"
TOKEN = secrets.token_urlsafe(24)
WIN_W, WIN_H = 1392, 903               # 기본 창 크기 (가로:세로 비율)
MAX_BODY = 2 * 1024 * 1024            # 화면 → API 요청 최대 크기
# 화면(HTML)에 적용하는 보안 정책: 이 앱 파일만 실행 · 외부 주소 접속/불러오기 금지 · 다른 페이지 안에 못 띄움
CSP = ("default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; "
       "font-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'")
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)


def write_crash(msg):
    try:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(time.strftime("[%Y-%m-%d %H:%M:%S] ") + msg + "\n")
    except Exception:
        pass


# ---------------------------------------------------------------- 엔진 ↔ 화면 연결
class Bridge:
    def __init__(self):
        self.lock = threading.Lock()
        self.data = core.load_config()
        # 바이옴 매크로 · 매크로(매크로 탭)는 프로그램을 켤 때마다 꺼진 상태로 시작
        if (self.data.get("biome") or {}).get("enabled") or self.data.get("macro_on"):
            self.data["biome"]["enabled"] = False
            self.data["macro_on"] = False
            try:
                core.save_config(self.data)
            except Exception:
                pass
        macro.set_ocr_mode(self.data.get("ocr_engine"))
        self.cfg = core.Config(dict(self.data))
        self.handler = core.Handler(self.cfg)
        self.thread = None
        self.seq = 0
        self.logs = deque(maxlen=1000)       # (seq, time, msg, color)
        self.events = deque(maxlen=300)      # (seq, entry)
        self.crash = None              # 로블록스가 갑자기 꺼졌을 때: {"until": 복귀 실행 시각}
        self.status = ("stopped", "중지됨")
        self.last_ping = 0.0

        core.Bus.on_log = self._on_log
        core.Bus.on_status = self._on_status
        core.Bus.on_event = self._on_event
        core.Bus.on_joined = self._on_joined
        # 바이옴 매크로: 로그는 항상 읽고(현재 바이옴 표시), 웹후크는 바이옴 매크로 토글이 켜져 있을 때만 (시작 버튼과 무관)
        self.biome = biome.BiomeWatcher(lambda: self.data.get("biome", {}),
                                        lambda: bool(self.data.get("biome", {}).get("enabled")),
                                        on_change=self._on_biome_change)
        # 오토 팝핑: Play 버튼 자동 클릭
        self.pop = popping.Popper(lambda: self.data.get("pop", {}), self._on_log, on_end=self._on_pop_end)
        # 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버)
        self.mpop = popping.MyServerPopper(lambda: self.data.get("pop", {}), lambda: self.data.get("mpop", {}),
                                           self._on_log)
        self.play = rejoin.PlayClicker(lambda: self.data.get("play", {}), self._on_log,
                                       on_ingame=self._on_ingame, on_fail=self._on_play_fail)
        # 매크로 복귀: 로블록스 전부 종료 → 1초 → 내 브섭 링크 → Play
        # 게임 접속 전 동작 (프로그램 강제 종료 · 키 입력 등) — 접속 직전에 실행하고 끝날 때까지 기다림
        self.pre = stepmod.StepRunner(lambda: self.data.get("play", {}).get("pre_steps", []), self._on_log,
                                      label="접속 전 동작")
        core.Bus.before_join = lambda url: self.pre.run_now("게임 접속 전")
        # 복귀 후 동작 (플레이어가 직접 만드는 매크로)
        self.steps = stepmod.StepRunner(lambda: self.data.get("ret", {}).get("steps", []), self._on_log)
        self.ret = rejoin.Returner(lambda: self.data.get("ret", {}), self._on_log, self.play,
                                   kill=core.kill_roblox, launch=core.open_link)
        self.biome.start()                  # 바이옴 변경 콜백이 위 실행기들을 쓰므로 맨 마지막에 시작

    # 엔진 콜백 (작업 스레드에서 옴)
    def _next(self):
        self.seq += 1
        return self.seq

    def _on_log(self, msg, color=""):
        with self.lock:
            self.logs.append((self._next(), time.time(), msg, color))

    def _on_status(self, state, text=""):
        with self.lock:
            self.status = (state, text or state)
        if state in ("flux", "dom"):   # 디스코드에 붙으면 이름 모르는 항목을 채움
            threading.Thread(target=self.resolve_names, daemon=True).start()

    def _known(self, i):
        return bool(self.data.get("names", {}).get(i) or self.data.get("channels", {}).get(i, {}).get("name"))

    def resolve_names(self, ids=None):
        with self.lock:
            if ids is None:
                ids = [i for i in self.data["guild_ids"] + self.data["channel_ids"] if not self._known(i)]
            gids, cids = set(self.data["guild_ids"]), set(self.data["channel_ids"])
        if not ids:
            return
        res = core.lookup_names(ids)
        if not res:
            return
        with self.lock:
            names = self.data.setdefault("names", {})
            chans = self.data.setdefault("channels", {})
            for i, r in res.items():
                if r.get("type") == "guild":
                    names[i] = r.get("name", "") + ("  (서버 ID임)" if i in cids else "")
                    if i in cids:   # 채널 목록에 서버 ID를 넣은 경우도 보이게
                        chans[i] = {"name": names[i], "guild": "", "guildId": i}
                else:
                    chans[i] = {"name": r.get("name", ""), "guild": r.get("guildName", ""),
                                "guildId": r.get("guildId")}
                    if i in gids:
                        names[i] = f"#{r.get('name', '')}  (채널 ID임)"
        self._save()
        self._on_log(f"이름 {len(res)}개를 디스코드에서 가져옴", "d")

    def _on_event(self, e):
        changed = False
        with self.lock:
            self.events.append((self._next(), e))
            names = self.data.setdefault("names", {})
            chans = self.data.setdefault("channels", {})
            if e.get("guildId") and e.get("guildName") and names.get(e["guildId"]) != e["guildName"]:
                names[e["guildId"]] = e["guildName"]
                changed = True
            if e.get("channelId") and e.get("channelName"):
                info = {"name": e["channelName"], "guild": e.get("guildName", ""), "guildId": e.get("guildId")}
                if chans.get(e["channelId"]) != info:
                    chans[e["channelId"]] = info
                    changed = True
        if changed:
            self._save()

    def _save(self):
        with self.lock:
            self.data = core.normalize(self.data)
            data = dict(self.data)
        macro.set_ocr_mode(data.get("ocr_engine"))
        self.cfg.set_data(data)
        try:
            core.save_config(data)
            self.cfg.mtime = core.CONFIG_PATH.stat().st_mtime
        except Exception as e:
            self._on_log(f"설정 저장 실패: {e}", "r")

    def running(self):
        return bool(self.thread and self.thread.is_alive())

    # ---------------- API ----------------
    def api_state(self, _):
        with self.lock:
            return {"config": self.data, "running": self.running(), "armed": core.ARMED.is_set(),
                    "status": list(self.status),
                    "count": self.handler.count, "version": VERSION}

    # ---------------- 로블록스가 갑자기 꺼짐 (매크로 작동 중) ----------------
    # 시작 버튼이 켜져 있고, 매크로가 일부러 끈 게 아닌데 로블록스 창이 사라지면:
    #   오른쪽 아래 알림 + 소리 → 10초 뒤 매크로 복귀. 그 전에 [대기] 를 누르면 일부러 끈 걸로 보고 복귀 안 함
    CRASH_WAIT = 10.0

    def _crash_watch(self):
        seen, gone_since = False, None
        while True:
            time.sleep(0.5)
            try:
                if getattr(self, "_crash_reset", False):   # [대기] 를 누름 → 로블록스가 다시 켜질 때까지 감시 안 함
                    self._crash_reset = False
                    seen, gone_since = False, None
                now = time.time()
                present = bool(macro.roblox_window_cached(0.5))
                busy = (self.ret.running() or self.play.running() or self.pop.running() or self.mpop.running()
                        or self.pre.running() or now < core.EXPECT_CLOSE["until"])
                if present:
                    seen, gone_since = True, None
                    if self.crash:                     # 다시 켜짐 → 알림 취소
                        self.crash = None
                    continue
                if not core.ARMED.is_set() or busy:
                    seen, gone_since = False, None     # 꺼진 상태 · 매크로가 일부러 끄는 중엔 감시 안 함
                    if self.crash and not core.ARMED.is_set():
                        self.crash = None
                    continue
                if self.crash:
                    if now >= self.crash["until"]:
                        self.crash = None
                        seen = False
                        self._on_log("로블록스가 다시 켜지지 않음 — 매크로 복귀 실행", "y")
                        self.ret.start("로블록스 꺼짐")
                    continue
                if not seen:
                    continue
                gone_since = gone_since or now
                if now - gone_since >= 1.5:            # 잠깐 사라졌다 다시 뜨는 경우는 무시
                    self.crash = {"until": now + self.CRASH_WAIT}
                    self._on_log(f"로블록스가 갑자기 꺼짐 — {self.CRASH_WAIT:g}초 뒤 매크로 복귀 (대기를 누르면 취소)", "y")
                    self._alarm()
            except Exception as e:
                write_crash(f"crash watch: {e}")

    @staticmethod
    def _alarm():
        try:
            import winsound
            winsound.PlaySound("SystemExclamation", winsound.SND_ALIAS | winsound.SND_ASYNC)
        except Exception:
            pass

    def api_crash_cancel(self, _):
        self._crash_reset = True
        if self.crash:
            self.crash = None
            self._on_log("복귀 취소 — 일부러 끈 것으로 보고 대기", "c")
        return {"ok": True}

    def api_poll(self, p):
        self.last_ping = time.time()
        since = int(p.get("since", 0))
        roblox = bool(macro.roblox_window_cached(2.0))     # 창 검색은 잠금 밖에서, 2초 동안 재사용
        play, bio, pop, ret = self.play.snapshot(), self.biome.state(), self.pop.snapshot(), self.ret.snapshot()
        mpop = self.mpop.snapshot()
        st = self.steps.snapshot()
        pre = self.pre.snapshot()
        with self.lock:
            logs = [{"seq": s, "time": t, "msg": m, "color": c} for s, t, m, c in self.logs if s > since]
            events = [dict(e, seq=s) for s, e in self.events if s > since]
            return {"seq": self.seq, "logs": logs, "events": events, "status": list(self.status),
                    "running": self.running(), "armed": core.ARMED.is_set(),
                    "play": play, "pop": pop, "ret": ret, "mpop": mpop, "steps": st, "pre": pre, "roblox": roblox,
                    "crash": {"left": max(0.0, self.crash["until"] - time.time())} if self.crash else None,
                    "biome": bio,
                    "count": self.handler.count, "names": self.data.get("names", {}),
                    "channels": self.data.get("channels", {})}

    def api_set_config(self, p):
        patch = p.get("patch") or {}
        allowed = set(core.DEFAULT_CONFIG) - {"names", "channels", "guild_ids", "channel_ids"}
        with self.lock:
            for k, v in patch.items():
                if k in allowed:
                    self.data[k] = v
        self._save()
        return {"config": self.data}

    def api_add_target(self, p):
        kind, i, name = p.get("kind"), str(p.get("id", "")).strip(), (p.get("name") or "").strip()
        guild, gid = (p.get("guild") or "").strip(), p.get("guildId")
        key = {"guild": "guild_ids", "channel": "channel_ids"}.get(kind)
        if not key or not i.isdigit():
            return {"error": "숫자 ID만 입력 가능"}
        with self.lock:
            if i not in self.data[key]:
                self.data[key].append(i)
            if name and kind == "guild":
                self.data.setdefault("names", {})[i] = name
            elif name:
                self.data.setdefault("channels", {})[i] = {"name": name, "guild": guild, "guildId": gid}
        self._save()
        if not name:
            if core.CONNECTED.is_set():
                self.resolve_names([i])     # 감시 중이면 바로 이름 가져옴
            else:
                return {"config": self.data, "note": "추가됨 · 디스코드 연결 후 이름을 가져옴"}
        return {"config": self.data}

    def api_remove_target(self, p):
        key = {"guild": "guild_ids", "channel": "channel_ids"}.get(p.get("kind"))
        i = str(p.get("id", ""))
        with self.lock:
            if key and i in self.data[key]:
                self.data[key].remove(i)
        self._save()
        return {"config": self.data}

    # ---------------- 오토 팝핑 (Play 버튼) ----------------
    def _on_joined(self, url):
        """로블록스 실행 직후: 항상 Play 버튼 클릭 시작 (위치가 없으면 로그만 남김)
        다른 사람 서버로 들어가는 것이라, 그 접속 동안은 바이옴 웹후크를 보내지 않음"""
        self.pop.stop()
        self.mpop.stop()
        self.ret.stop()
        self.biome.mute_next_session()
        self.play.start("서버 접속")

    def _on_ingame(self, path, reason):
        if reason == "서버 접속":           # 스나이핑 접속 → 오토 팝핑
            self.pop.start(log_path=path)
        elif reason == "복귀":
            self._on_log("매크로 복귀 완료 — 내 서버 입장", "g")
            wait = float(self.data.get("ret", {}).get("start_wait", 7.5))
            self.steps.start("복귀 완료", delay=wait)  # 복귀 후 동작이 있으면 입장 후 대기(기본 7.5초) 뒤 실행

    def _on_play_fail(self, reason):
        if reason == "서버 접속":           # 스나이핑한 서버에 못 들어감 → 복귀
            self.ret.start("접속 실패")

    def _on_biome_change(self, prev, found, sniping):
        """매크로 탭 · 레어 바이옴 자동 팝핑: 지금 켜져 있는 로블록스(내 서버)에서 레어 바이옴이 시작되면 포션 사용
        스나이핑으로 들어간 다른 사람 서버이거나, 오토 팝핑 · 복귀 · Play 클릭이 도는 중이면 안 함"""
        mp = self.data.get("mpop", {})
        if not self.data.get("macro_on") or not mp.get("enabled") or found not in popping.POP_BIOMES:
            return
        if sniping:
            self._on_log(f"{found} 감지 — 스나이핑 접속이라 내 서버 팝핑 안 함", "d")
            return
        if self.pop.running() or self.ret.running() or self.play.running() or self.pre.running():
            self._on_log(f"{found} 감지 — 다른 매크로가 도는 중이라 내 서버 팝핑 안 함", "y")
            return
        if not (mp.get("biomes_on") or {}).get(found, True):
            self._on_log(f"{found} 감지 — 매크로 탭 팝핑 바이옴에서 꺼져 있음", "y")
            return
        if not macro.roblox_window_cached(1.0):
            return                          # 옛 로그 파일 (로블록스가 꺼져 있음)
        self.mpop.start(found)

    def _on_pop_end(self, stopped):
        if not stopped:                     # 바이옴 종료·접속 끊김 등으로 끝남 → 복귀 (직접 멈춘 경우 제외)
            self.ret.start("오토 팝핑 종료")

    def api_steps_run(self, p):
        self.pop.stop()
        runner = self.pre if p.get("which") == "pre" else self.steps
        ok = runner.start("테스트", delay=2)
        return {"ok": True} if ok else {"error": "켜진 동작이 없음"}

    def api_steps_info(self, _):
        return {"keys": macro.KEY_NAMES, "types": stepmod.STEP_TYPES}

    def api_combo_check(self, p):
        try:
            return {"keys": stepmod.parse_combo(p.get("keys"))}
        except ValueError as e:
            return {"error": str(e)}

    def api_list_windows(self, _):
        seen, out = set(), []
        for w in macro.list_windows():
            k = (w["title"], w["exe"])
            if k not in seen and w["title"].strip():
                seen.add(k)
                out.append({"title": w["title"], "exe": w["exe"]})
        return {"windows": out}

    def api_pick_point_overlay(self, _):
        return self._pick_overlay("--pick-point")

    def api_pick_file(self, _):
        return self._pick_overlay("--pick-file")

    def api_ret_join(self, _):
        """로블록스가 꺼져 있을 때 메인 알림에서: 매크로 복귀 설정의 내 브섭 링크로 바로 접속"""
        link = (self.data.get("ret", {}).get("ps_link") or "").strip()
        if not link:
            return {"error": "매크로 복귀 설정에 내 브섭 링크가 없음"}
        try:
            core.open_link(link)
        except Exception as e:
            return {"error": f"실행 실패: {e}"}
        return {"ok": True}

    def api_ret_test(self, _):
        if not (self.data.get("ret", {}).get("ps_link")):
            return {"error": "복귀할 브섭 링크 먼저 입력"}
        self.pop.stop()
        return {"ok": self.ret.start("테스트")} if not self.ret.running() else {"error": "이미 복귀 중"}

    def api_play_test(self, _):
        pl = self.data.get("play", {})
        if not pl.get("pos"):
            return {"error": "Play 버튼 위치 먼저 지정"}
        if not pl.get("skip_pos"):
            return {"error": "Click to skip 버튼 위치 먼저 지정"}
        self.play.start("테스트", load_wait=1)
        return {"ok": True}

    def api_play_stop(self, _):
        self.play.stop()
        self.pop.stop()
        self.ret.stop()
        self.steps.stop()
        self.pre.stop()
        return {"ok": True}

    # 오토 팝핑 (레어 바이옴 포션 사용)
    def api_pop_pos(self, p):
        """버튼 위치 지정 — key: inventory_pos / items_pos / search_pos / item_pos / amount_pos / use_pos"""
        key = str(p.get("key", ""))
        if key not in dict(popping.POS_KEYS):
            return {"error": "알 수 없는 항목"}
        r = self._pick_overlay("--pick-point")
        if r.get("error"):
            return r
        with self.lock:
            self.data.setdefault("pop", {})[key] = [round(r["x"], 4), round(r["y"], 4)]
        self._save()
        return {"pos": self.data["pop"][key]}

    def api_pop_region(self, _):
        r = self._pick_overlay("--pick-region")
        if r.get("error"):
            return r
        with self.lock:
            self.data.setdefault("pop", {})["ocr_region"] = r["region"]
        self._save()
        return {"region": r["region"]}

    def api_pop_ocr_test(self, p):
        region = self.data.get("pop", {}).get("ocr_region")
        if not region:
            return {"error": "OCR 영역 먼저 지정"}
        try:
            text = macro.ocr_region(region, item=True)
        except ModuleNotFoundError:
            return {"error": "OCR 패키지가 설치되지 않음 (run.bat 으로 실행 필요)"}
        except Exception as e:
            return {"error": f"OCR 오류: {e}"}
        name, count = popping.parse_ocr(text)
        target = str(p.get("name") or "")
        score = popping.similarity(name, target) if target else None
        self._on_log(f"OCR 테스트: '{text[:60]}' → 이름 '{name}' · 개수 {count}"
                     + (f" · '{target}' 일치율 {score:g}%" if target else ""), "d")
        return {"text": text, "name": name, "count": count, "score": score, "engine": macro.ocr_engine_name()}

    def api_pop_default(self, p):
        """바이옴 템플릿을 기본값(스크립트의 레어 바이옴 자동 팝핑)으로 되돌림"""
        b = str(p.get("biome", "")).upper()
        if b not in core.POP_TEMPLATES:
            return {"error": "알 수 없는 바이옴"}
        tpl = json.loads(json.dumps(core.POP_TEMPLATES[b]))
        with self.lock:
            self.data.setdefault("pop", {}).setdefault("templates", {})[b] = tpl
        self._save()
        return {"template": self.data["pop"]["templates"][b]}

    def api_pop_test(self, p):
        b = str(p.get("biome", "")).upper()
        cfg = self.data.get("pop", {})
        miss = popping.Popper.missing(cfg)
        if miss:
            return {"error": "설정 필요: " + ", ".join(miss)}
        if not [i for i in ((cfg.get("templates") or {}).get(b) or {}).get("items", []) if i.get("name")]:
            return {"error": "포션 목록이 비어 있음"}
        self.play.stop()
        self.pop.start(test_biome=b)
        return {"ok": True}

    # ---------------- 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버) ----------------
    def api_mpop_default(self, p):
        b = str(p.get("biome", "")).upper()
        if b not in core.POP_TEMPLATES:
            return {"error": "알 수 없는 바이옴"}
        tpl = json.loads(json.dumps(core.POP_TEMPLATES[b]))
        with self.lock:
            self.data.setdefault("mpop", {}).setdefault("templates", {})[b] = tpl
        self._save()
        return {"template": self.data["mpop"]["templates"][b]}

    def api_mpop_test(self, p):
        b = str(p.get("biome", "")).upper()
        miss = popping.Popper.missing(self.data.get("pop", {}))
        if miss:
            return {"error": "오토 팝핑 설정 필요: " + ", ".join(miss)}
        if not [i for i in ((self.data.get("mpop", {}).get("templates") or {}).get(b) or {}).get("items", [])
                if i.get("name")]:
            return {"error": "포션 목록이 비어 있음"}
        self.mpop.start(b, test=True)
        return {"ok": True}

    def api_mpop_stop(self, _):
        self.mpop.stop()
        return {"ok": True}

    def api_play_pos(self, p):
        """로블록스 화면 위에서 클릭 1번으로 위치 지정 — which: play (Play 버튼) / skip (Click to skip)"""
        key = "skip_pos" if p.get("which") == "skip" else "pos"
        r = self._pick_overlay("--pick-point")
        if r.get("error"):
            return r
        with self.lock:
            self.data.setdefault("play", {})[key] = [round(r["x"], 4), round(r["y"], 4)]
        self._save()
        return {"pos": self.data["play"][key]}

    # ---------------- 바이옴 매크로 ----------------
    def api_webhook_test(self, _):
        b = self.data.get("biome", {})
        hooks = [h for h in b.get("webhooks", []) if h]
        if not hooks:
            return {"error": "웹후크 주소 없음"}
        bad = [h for h in hooks if not biome.is_webhook(h)]
        if bad:
            return {"error": "디스코드 웹후크 주소 형식이 아님"}
        res = biome.send_all(hooks, biome.test_payload(b.get("player"), b.get("ps_link")), wait=True)
        ok = sum(1 for _, r in res if r is True)
        return {"ok": ok, "total": len(hooks), "errors": [r for _, r in res if r is not True]}

    def api_pick_point(self, p):
        return macro.pick_point(p.get("key", "f8"), float(p.get("timeout", 30))) or {"error": "취소됨"}

    def api_pick_region(self, _):
        return self._pick_overlay("--pick-region")

    def _pick_overlay(self, flag):
        """로블록스 화면 위 선택 창 (tkinter 창이라 별도 프로세스로 띄우고 결과는 임시 파일로 받음)
        --pick-region: 드래그로 영역 / --pick-point: 클릭 1번으로 위치"""
        import tempfile
        out = Path(tempfile.gettempdir()) / f"acrux_pick_{os.getpid()}.json"
        out.unlink(missing_ok=True)
        if getattr(sys, "frozen", False):
            cmd = [sys.executable, flag, str(out)]
        else:
            cmd = [sys.executable, str(Path(__file__).resolve().parent / "macro.py"), flag, str(out)]
        env = dict(os.environ, ACRUX_LANG=str(self.data.get("lang") or "ko"))   # 안내 글자 언어
        try:
            subprocess.run(cmd, timeout=300 if flag == "--pick-file" else 150, creationflags=NO_WINDOW, env=env)
            if not out.exists():
                return {"error": "지정 실패"}
            return json.loads(out.read_text(encoding="utf-8"))
        except subprocess.TimeoutExpired:
            return {"error": "시간 초과"}
        except Exception as e:
            return {"error": str(e)}
        finally:
            out.unlink(missing_ok=True)

    def api_ocr_test(self, p):
        region = p.get("region")
        if not region or len(region) != 4:
            return {"error": "영역이 없음"}
        try:
            text = macro.ocr_region(region)
            core.log(f"OCR 테스트: {text[:60] or '(읽은 글자 없음)'}", "d")
            return {"text": text}
        except ModuleNotFoundError as e:
            core.log(f"OCR 패키지 없음: {e}", "r")
            return {"error": "OCR 패키지가 설치되지 않음 (run.bat 으로 실행 필요)"}
        except Exception as e:
            core.log(f"OCR 오류: {type(e).__name__}: {e}", "r")
            return {"error": f"OCR 오류: {e}"}

    # 감지 엔진은 앱이 켜지면 바로 돌기 시작 (정보를 바로 불러오게)
    def start_engine(self):
        if not getattr(self, "_crash_thread", None):
            self._crash_thread = threading.Thread(target=self._crash_watch, daemon=True)
            self._crash_thread.start()
        if self.running():
            return
        core.STOP.clear()
        self.thread = threading.Thread(target=core.run, args=(self.cfg, self.handler), daemon=True)
        self.thread.start()
        self._on_status("launching", "디스코드 연결 중")

    # 시작/중지 버튼은 실제 작동(링크 열기 등)만 켜고 끔
    def pop_missing(self):
        """오토 팝핑 필수 설정 중 비어 있는 것 (Play / Click to skip 위치 + 버튼 6개 + OCR 영역)"""
        pl = self.data.get("play", {})
        miss = [n for k, n in (("pos", "Play 버튼"), ("skip_pos", "Click to skip 버튼")) if not pl.get(k)]
        return miss + popping.Popper.missing(self.data.get("pop", {}))

    def api_start(self, _):
        miss = self.pop_missing()
        if miss:                                  # 오토 팝핑 설정을 안 하면 매크로를 켜지 않음
            self._on_log(f"시작 안 됨 — 오토 팝핑 설정 필요: {', '.join(miss)}", "y")
            return {"error": "오토 팝핑 설정 필요", "missing": miss}
        if not core.ARMED.is_set():
            core.ARMED.set()
            self._on_log("작동 시작", "c")
        self.start_engine()
        return {"ok": True}

    def api_stop(self, _):
        if core.ARMED.is_set():
            core.ARMED.clear()
            self._on_log("작동 중지 — 감지만 계속", "c")
        return {"ok": True}

    def api_open_url(self, p):
        url = str(p.get("url", ""))
        if core.LINK_RE.fullmatch(url):
            # 직접 [열기] 를 누른 것: 대기 없이 바로 (링크 여는 방식만 설정을 따름)
            core.join_server(url, self.data.get("stable_join", True), self.data.get("snipe"), manual=True)
        return {"ok": True}

    # 크레딧 링크 (정해진 주소만 열기)
    WEB_LINKS = {
        "discord": "https://discord.gg/cyK26gsjCC",
        "roblox": "https://www.roblox.com/ko/users/11743130211/profile",
        "donate": "https://ko-fi.com/rn_draon",
    }

    def api_open_web(self, p):
        url = self.WEB_LINKS.get(str(p.get("site", "")))
        if not url:
            return {"error": "준비 중"}
        import webbrowser
        webbrowser.open(url)
        return {"ok": True}

    def api_uninstall(self, _):
        """Acrux 완전 삭제: 창을 닫고 %LOCALAPPDATA%\\AcruxMacro (앱·런타임·설정·기록·화면 프로필) 를 통째로 지움.
        실행기(Acrux.exe)는 그대로 — 다시 켜면 처음부터 새로 설치됨.
        이 프로그램(런타임)도 그 폴더 안에 있어서 직접은 못 지움 → 꺼진 뒤에 지우는 cmd 를 따로 띄움"""
        root = Path(os.environ.get("ACRUX_ROOT") or (Path(os.environ.get("LOCALAPPDATA", "")) / APP_NAME))
        if root.name.lower() != APP_NAME.lower() or not os.environ.get("LOCALAPPDATA"):
            return {"error": "삭제할 폴더를 찾을 수 없음"}
        self._on_log("Acrux 완전 삭제 — 창을 닫고 데이터를 지웁니다", "r")
        # 프로그램·화면이 완전히 꺼질 때까지 1초마다 최대 30번 다시 시도하는 배치 파일 (끝나면 자기 자신도 지움)
        # 경로는 %LOCALAPPDATA% 로 써서 사용자 이름에 한글이 있어도 문제없게
        bat = Path(os.environ.get("TEMP") or os.environ.get("TMP") or str(Path.home())) / f"acrux_uninstall_{os.getpid()}.cmd"
        target = r"%LOCALAPPDATA%\AcruxMacro"
        bat.write_text("\r\n".join([
            "@echo off",
            "for /l %%i in (1,1,30) do (",
            "  ping -n 2 127.0.0.1 >nul",
            f'  rmdir /s /q "{target}" 2>nul',
            f'  if not exist "{target}" goto done',
            ")",
            ":done",
            'del "%~f0"',
        ]) + "\r\n", encoding="ascii")
        try:
            subprocess.Popen(["cmd", "/d", "/c", str(bat)], creationflags=NO_WINDOW, close_fds=True,
                             cwd=str(bat.parent))
        except Exception as e:
            return {"error": f"실행 실패: {e}"}

        def bye():
            time.sleep(0.6)                       # 화면이 응답을 받을 시간
            core.STOP.set()
            if self.thread:
                self.thread.join(15)
            core.close_debug_port(self.cfg)       # 디스코드를 보통 모드로 (디버그 포트 닫기)
            p = getattr(self, "edge_proc", None)
            if p and p.poll() is None:
                subprocess.run(["taskkill", "/PID", str(p.pid), "/T", "/F"], capture_output=True, creationflags=NO_WINDOW)
            os._exit(0)
        threading.Thread(target=bye, daemon=True).start()
        return {"ok": True}

    def api_ocr_info(self, _):
        """Acrux 설정 · OCR 감지 방식: 지금 쓰는 엔진 · RapidOCR 설치 여부"""
        rapid = macro.rapid_engine() is not None
        return {"engine": macro.ocr_engine_name(), "rapid": rapid, "mode": macro.OCR_MODE["mode"],
                "rapid_error": None if rapid else macro._RAPID.get("failed")}

    def api_open_folder(self, _):
        os.startfile(str(core.DATA_BASE))      # 설정·로그가 있는 폴더
        return {"ok": True}


BRIDGE = None

MIME = {".gif": "image/gif", ".png": "image/png", ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8",
        ".js": "application/javascript; charset=utf-8", ".svg": "image/svg+xml", ".png": "image/png",
        ".ico": "image/x-icon", ".woff2": "font/woff2"}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype="application/json; charset=utf-8"):
        if isinstance(body, (dict, list)):
            body = json.dumps(body, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("X-Frame-Options", "DENY")
        self.send_header("Referrer-Policy", "no-referrer")
        if ctype.startswith("text/html"):
            self.send_header("Content-Security-Policy", CSP)
        self.end_headers()
        self.wfile.write(body)

    def _host_ok(self):
        # DNS 리바인딩 차단: 주소창 이름이 127.0.0.1 / localhost 일 때만
        return (self.headers.get("Host") or "").split(":")[0] in ("127.0.0.1", "localhost")

    def _origin_ok(self):
        # 다른 사이트에서 보낸 요청 차단 (화면은 같은 주소에서만 보냄)
        o = self.headers.get("Origin")
        port = self.server.server_address[1]
        return o is None or o in (f"http://127.0.0.1:{port}", f"http://localhost:{port}")

    def do_GET(self):
        if not self._host_ok():
            return self._send(403, {"error": "forbidden"})
        path = self.path.split("?")[0].split("#")[0]
        if path == "/":
            path = "/index.html"
        f = (WEB_DIR / path.lstrip("/")).resolve()
        if WEB_DIR.resolve() not in f.parents or not f.is_file():
            return self._send(404, {"error": "not found"})
        self._send(200, f.read_bytes(), MIME.get(f.suffix, "application/octet-stream"))

    def do_POST(self):
        token = (self.headers.get("X-Token") or "").encode("utf-8", "ignore")
        if not self._host_ok() or not self._origin_ok() or not secrets.compare_digest(token, TOKEN.encode()):
            return self._send(403, {"error": "forbidden"})
        name = self.path.split("?")[0].removeprefix("/api/")
        fn = getattr(BRIDGE, "api_" + name, None) if re.fullmatch(r"[a-z_]{1,40}", name) else None
        if not fn:
            return self._send(404, {"error": "unknown api"})
        try:
            n = int(self.headers.get("Content-Length") or 0)
            if n < 0 or n > MAX_BODY:
                return self._send(413, {"error": "too large"})
            payload = json.loads(self.rfile.read(n) or b"{}") if n else {}
            if not isinstance(payload, dict):
                return self._send(400, {"error": "bad request"})
            self._send(200, fn(payload) or {})
        except Exception as e:
            write_crash(traceback.format_exc())
            self._send(500, {"error": str(e)})


# ---------------------------------------------------------------- Edge 앱 창
def find_edge():
    cands = []
    for base in (os.environ.get("ProgramFiles(x86)"), os.environ.get("ProgramFiles"), os.environ.get("LOCALAPPDATA")):
        if base:
            cands.append(Path(base) / "Microsoft" / "Edge" / "Application" / "msedge.exe")
    try:
        import winreg
        for root in (winreg.HKEY_LOCAL_MACHINE, winreg.HKEY_CURRENT_USER):
            try:
                with winreg.OpenKey(root, r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe") as k:
                    cands.insert(0, Path(winreg.QueryValue(k, None)))
            except OSError:
                pass
    except ImportError:
        pass
    return next((c for c in cands if c.is_file()), None)


def main():
    global BRIDGE
    BRIDGE = Bridge()
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    port = server.server_address[1]
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{port}/#{TOKEN}"

    BRIDGE.start_engine()                 # 앱 켜자마자 디스코드 연결 + 감지
    # OCR 엔진은 미리 불러둠 (첫 OCR 이 느리지 않게)
    threading.Thread(target=lambda: BRIDGE._on_log(f"OCR 엔진: {macro.ocr_engine_name()}", "d"), daemon=True).start()
    if BRIDGE.data.get("autostart"):      # '바로 작동' 설정이면 시작 버튼까지 눌린 상태로
        BRIDGE.api_start({})

    edge = find_edge()
    proc = None
    if edge:
        DATA_DIR.mkdir(parents=True, exist_ok=True)
        proc = subprocess.Popen([str(edge), f"--app={url}", f"--user-data-dir={DATA_DIR / 'ui'}",
                                 f"--window-size={WIN_W},{WIN_H}", "--no-first-run", "--no-default-browser-check",
                                 "--disable-features=Translate,msEdgeSidebarV2", "--disable-extensions"])
    else:
        import webbrowser
        webbrowser.open(url)
    BRIDGE.edge_proc = proc
    try:                                  # 창 · 작업 표시줄 아이콘을 Acrux 아이콘으로
        import winicon
        # 기본 창 크기는 이 크기 비율로 한 번만 맞춤 (그 뒤로 직접 바꾼 크기는 Edge 가 기억)
        fit = None if BRIDGE.data.get("win_size") == f"{WIN_W}x{WIN_H}" else (WIN_W, WIN_H)
        if fit:
            BRIDGE.data["win_size"] = f"{WIN_W}x{WIN_H}"
            BRIDGE._save()
        winicon.keep(str(Path(__file__).resolve().parent / "icon.ico"), alive=lambda: not core.STOP.is_set(), fit=fit)
    except Exception as e:
        write_crash(f"window icon: {e}")

    # 창이 닫히면 종료: Edge 프로세스가 끝났고, 화면에서 오는 신호도 끊겼을 때
    started = time.time()
    while True:
        time.sleep(1)
        alive = proc is not None and proc.poll() is None
        silent = time.time() - (BRIDGE.last_ping or started) > (8 if BRIDGE.last_ping else 60)
        if not alive and silent:
            break

    shutdown(server)


def shutdown(server=None):
    """프로그램을 끌 때: 감지 중지 → 디스코드를 보통 모드로 다시 켬 (디버그 포트가 계속 열려 있지 않게)"""
    core.STOP.set()
    if BRIDGE and BRIDGE.thread:
        BRIDGE.thread.join(15)                # 감지 엔진이 완전히 멈춘 뒤에 디스코드를 건드림
    if server:
        server.shutdown()
    if BRIDGE:
        core.close_debug_port(BRIDGE.cfg)


if __name__ == "__main__":
    if "--pick-region" in sys.argv:          # exe 로 묶었을 때 영역 지정 창으로 다시 실행되는 경우
        macro.pick_region_to_file(sys.argv[sys.argv.index("--pick-region") + 1])
        sys.exit(0)
    if "--pick-file" in sys.argv:            # 프로그램 고르기 창
        macro.pick_file_to_file(sys.argv[sys.argv.index("--pick-file") + 1])
        sys.exit(0)
    if "--pick-point" in sys.argv:           # 클릭 위치 지정 창
        macro.pick_region_to_file(sys.argv[sys.argv.index("--pick-point") + 1], "point")
        sys.exit(0)
    try:
        main()
    except Exception:
        write_crash(traceback.format_exc())
        try:
            shutdown()
        except Exception:
            pass
        raise
