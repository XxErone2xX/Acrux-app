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
import tempfile
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
import fishing
import move
import sell
import merchant
from version import VERSION


class _AutocalStop(Exception):
    """자동 보정을 멈춤 (취소 · 더 진행할 수 없음) — 그때까지 잰 위치는 저장됨"""

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
        self.pop = popping.Popper(self._pop_cfg, self._on_log, on_end=self._on_pop_end)
        # 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버)
        # 매크로 탭 · 자동 낚시 (제자리 낚시) — 매크로 버튼 + 켜기가 켜져 있는 동안 계속
        self.fisher = fishing.Fisher(self._mfish_cfg, self._on_log,
                                     on_user_stop=self._macro_user_stop)
        self.mpop = popping.MyServerPopper(lambda: self.data.get("pop", {}), self._mpop_cfg,
                                           self._on_log, before=lambda: self.fisher.hold(45),
                                           after=self.fisher.release)
        # 이동: 기준 장소(리셋 · 카메라 정렬 · 줌) → 화면의 한 점을 눌러 장소로 걸어감 · 걸리는 시간은 직접 잼
        self.mover = move.Mover(lambda: self.data.get("base", {}), lambda: self.data.get("move", {}), self._on_log,
                                set_move_time=self._set_move_time, before=lambda: self.fisher.hold(45),
                                after=self.fisher.release, banner=self._move_banner,
                                progress=self._banner_progress)
        # 판매: 자동 낚시 중 인벤토리가 가득 차면 물고기 판매 장소로 가서 팔고 낚시 장소로 돌아옴
        self.seller = sell.Seller(self.mover, self._mfish_cfg, lambda: self.data.get("move", {}), self._on_log)
        self.fisher.on_full = self._on_fish_full
        # 오토 아이템 사용: 쿨타임마다 Strange Controller · Biome Randomizer 사용 (자동 낚시는 잠깐 비켜줌)
        self.items = popping.ItemUser(lambda: self.data.get("pop", {}), lambda: self.data.get("base", {}),
                                      lambda: self.data.get("mitem", {}), self._on_log,
                                      before=lambda: self.fisher.hold(45), after=self.fisher.release)
        # 상인 자동 구매: 일정 간격마다 채팅 확인 → 상인이 오면 Merchant Teleporter 로 가서 구매 (자동 낚시는 멈췄다가 다시 시작)
        self.merchant = merchant.Merchant(lambda: self.data.get("pop", {}), lambda: self.data.get("base", {}),
                                          lambda: self.data.get("mmerch", {}), self._on_log, on_done=self._on_merchant_done,
                                          before=lambda: self.fisher.hold(20), after=self.fisher.release,
                                          on_cal=self._on_merchant_cal)
        self.merchant_pending, self.merchant_check_at = None, 0.0
        self.mpop_wait = False             # 레어 바이옴 팝핑이 다른 기능이 끝나길 기다리는 중
        self.fisher.on_start = self._on_fish_start
        self.play = rejoin.PlayClicker(lambda: self.data.get("play", {}), self._on_log,
                                       on_ingame=self._on_ingame, on_fail=self._on_play_fail)
        # 매크로 복귀: 로블록스 전부 종료 → 1초 → 내 브섭 링크 → Play
        # 스나이핑하는 동안은 매크로(자동 낚시 등)가 쉬고, 내 서버로 돌아오면 입장 후 대기 뒤 다시 켜짐
        self.resume_at = 0.0                # 이 시각 전엔 매크로를 다시 켜지 않음 (복귀 직후 게임이 다 뜰 때까지)
        self.ret = rejoin.Returner(lambda: self.data.get("ret", {}), self._on_log, self.play,
                                   kill=core.kill_roblox, launch=core.open_link)
        self.biome.start()                  # 바이옴 변경 콜백이 위 실행기들을 쓰므로 맨 마지막에 시작
        threading.Thread(target=self._macro_loop, daemon=True).start()
        threading.Thread(target=self._hotkey_loop, daemon=True).start()
        self.online = None                  # 지금 Acrux 를 켜 둔 사람 수 (서버에서 받음 · 모르면 None)
        threading.Thread(target=self._online_loop, daemon=True).start()

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
    # ---- 기능별 설정 + 매크로 기준 위치 (여러 기능이 같이 쓰는 위치는 base 에 하나만 저장)
    def _mpop_cfg(self):
        """레어 바이옴 자동 팝핑 (내 서버) 설정 + 기준 위치의 인벤토리 위치 · OCR 영역"""
        b = self.data.get("base", {})
        return dict(self.data.get("mpop", {}), **{k: b.get(k) for k in (*core.BASE_INV_KEYS, "ocr_region")})

    def _pop_cfg(self):
        """스나이프 탭 오토 팝핑 설정 — '매크로 기준 위치 사용'이 켜져 있으면 인벤토리 위치 · OCR 영역은 기준 위치로"""
        pop, b = self.data.get("pop", {}), self.data.get("base", {})
        first, other = (b, pop) if pop.get("use_base") else (pop, b)
        # 고른 쪽에 비어 있으면 다른 쪽 값을 씀 (통합 위치에만 지정해 두고 연동을 안 켠 경우 등 — 같은 게임 화면 위치)
        return dict(pop, **{k: first.get(k) or other.get(k) for k in (*core.BASE_INV_KEYS, "ocr_region")})

    def _mfish_cfg(self):
        """자동 낚시 설정 + 통합 위치의 알림 영역 · 대화창"""
        b = self.data.get("base", {})
        return dict(self.data.get("mfish", {}), notice_region=b.get("notice_region"), dialog_pos=b.get("dialog_pos"))

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
                        or now < core.EXPECT_CLOSE["until"])
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
        mfish = self.fisher.snapshot()
        mv = self.mover.snapshot()
        with self.lock:
            # 새로 생긴 것만 (번호가 늘어나는 순서로 쌓여 있어서 뒤에서부터 보다가 멈춤 — 0.3초마다 1000개를 다 훑지 않게)
            logs = []
            for s, t, m, c in reversed(self.logs):
                if s <= since:
                    break
                logs.append({"seq": s, "time": t, "msg": m, "color": c})
            logs.reverse()
            events = []
            for s, e in reversed(self.events):
                if s <= since:
                    break
                events.append(dict(e, seq=s))
            events.reverse()
            return {"seq": self.seq, "logs": logs, "events": events, "status": list(self.status),
                    "running": self.running(), "armed": core.ARMED.is_set(),
                    "play": play, "pop": pop, "ret": ret, "mpop": mpop, "mfish": mfish, "roblox": roblox,
                    "move": mv, "mitem": self.items.snapshot(), "mmerch": self.merchant.snapshot(), "macro_on": bool(self.data.get("macro_on")), "online": self.online, "online_on": bool(self.ONLINE_URL),
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
        self.mover.stop()
        self.fisher.stop()
        self.items.stop()
        self.merchant.stop()
        self.ret.stop()
        self.biome.mute_next_session()
        self.play.start("서버 접속")

    def _on_ingame(self, path, reason):
        if reason == "서버 접속":           # 스나이핑 접속 → 오토 팝핑
            self.pop.start(log_path=path)
        elif reason == "복귀":
            wait = float(self.data.get("ret", {}).get("start_wait", 7.5))
            self.resume_at = time.time() + wait      # 게임이 다 뜰 때까지 기다렸다가 매크로 다시 시작
            self._on_log(f"매크로 복귀 완료 — 내 서버 입장 · {wait:g}초 뒤 매크로 다시 시작", "g")

    def _on_play_fail(self, reason):
        if reason == "서버 접속":           # 스나이핑한 서버에 못 들어감 → 복귀
            self.ret.start("접속 실패")

    # ---------------- 매크로 탭 · 자동 낚시 ----------------
    def _macro_loop(self):
        """1초마다: 매크로 버튼 + 자동 낚시 켜기 + 로블록스 창 있음 + 내 서버(스나이핑 아님)
        + 스나이핑 쪽 동작(오토 팝핑 · 복귀 · Play 클릭 · 복귀 후 대기) 없음 → 자동 낚시 돌림, 아니면 멈춤 (돌아오면 다시 켜짐)"""
        warned, paused = None, False
        while True:
            time.sleep(1.0)
            try:
                mf = self.data.get("mfish", {})
                mi = self.data.get("mitem", {})
                want_items = bool(self.data.get("macro_on") and mi.get("enabled"))
                mm = self.data.get("mmerch", {})
                want_merch = bool(self.data.get("macro_on") and mm.get("enabled"))
                want = bool(self.data.get("macro_on") and mf.get("enabled")) or want_items or want_merch
                busy = self.pop.running() or self.ret.running() or self.play.running() or time.time() < self.resume_at
                sniping = busy or self.biome.muted()
                if want and sniping and not paused:
                    paused = True
                    self._on_log("스나이핑 중 — 매크로 잠시 멈춤 (내 서버로 돌아오면 다시 시작)", "c")
                elif paused and (not want or not sniping):
                    paused = False
                    if want:
                        self._on_log("내 서버 — 매크로 다시 시작", "g")
                ok = want and not sniping and bool(macro.roblox_window_cached(2.0))
                # 오토 아이템 사용: 쿨타임이 찼고 이동 · 판매 · 팝핑 중이 아니면 (낚시는 안전한 곳에서 잠깐 비켜줌)
                # 상인 자동 구매: 상인이 왔으면 낚시를 멈추고 구매 · 아니면 간격마다 채팅 확인
                others = self.items.running() or (self.mpop.running() or self.mpop_wait) or self.mover.running() or self.merchant.running()
                if self.merchant_pending and time.time() - self.merchant_pending[1] > 180:
                    self.merchant_pending = None             # 상인이 떠났을 시간
                if ok and want_merch and not others:
                    if self.merchant_pending:
                        if self.fisher.running():
                            self.fisher.stop()               # 상인한테 갔다가 낚시 장소로 다시 가야 해서 낚시는 끝냄
                        else:
                            name = self.merchant_pending[0]
                            self.merchant_pending = None
                            self.merchant.start_job("buy", name)
                        continue
                    if time.time() - self.merchant_check_at >= float(mm.get("check_sec", 30)):
                        self.merchant_check_at = time.time()
                        self.merchant.start_job("check")
                elif self.merchant.running() and self.merchant.job == "buy" and (sniping or not want_merch):
                    self.merchant.stop()
                if ok and want_items and not self.items.running() and not (self.mpop.running() or self.mpop_wait) and not self.mover.running() \
                        and not self.merchant.running() and not self.merchant_pending:
                    self.items.start(self.items.due())
                elif self.items.running() and not self.items.test and (sniping or not want_items):
                    self.items.stop()
                want = bool(self.data.get("macro_on") and mf.get("enabled"))
                ok = ok and want
                if want and fishing.Fisher.missing(mf):
                    ok = False
                    miss = ", ".join(fishing.Fisher.missing(mf))
                    if warned != miss:
                        warned = miss
                        self._on_log(f"자동 낚시 안 함 — 매크로 기준 위치 설정 필요: {miss}", "n")
                else:
                    warned = None
                if ok and not self.fisher.running() and not (self.mpop.running() or self.mpop_wait) and not self.mover.running() \
                        and not self.items.running() and not self.merchant.running() and not self.merchant_pending:
                    self.fisher.start()
                elif not ok and self.fisher.running():
                    self.fisher.stop()
            except Exception as e:
                write_crash(f"macro loop: {e}")

    # 사용자 수: Acrux 가 켜져 있는 동안 몇 분마다 '켜져 있음' 신호 (설치마다 만든 무작위 번호만 보냄)
    # 집계에 참여하지 않으면 신호 없이 숫자만 받아 봄 · 서버 코드는 server/online
    ONLINE_URL = "https://acrux.pentagration.com/"   # 사용자 수 서버 (Server hosted by shebern_park)

    def _online_loop(self):
        """1분마다 /count 로 사용자 수를 읽어 표시 · 집계에 참여하면 서버가 알려준 간격(기본 5분)마다 /ping 신호
        (신호 응답의 숫자는 서버에 따라 신호를 반영하기 전 값일 수 있어서, 표시는 항상 /count 로)"""
        import urllib.request
        time.sleep(3)
        ping_every, last_ping = 300, 0.0

        def call(path, body=None):
            req = urllib.request.Request(self.ONLINE_URL.rstrip("/") + path, data=body, method="POST" if body else "GET",
                                         headers={"Content-Type": "application/json", "User-Agent": f"Acrux/{VERSION}"})
            with urllib.request.urlopen(req, timeout=10) as r:
                return json.loads(r.read(4096).decode("utf-8"))
        while True:
            if self.ONLINE_URL:
                try:
                    if self.data.get("online_share", True) and time.time() - last_ping >= ping_every:
                        res = call("/ping", json.dumps({"id": self.data.get("install_id", "")}).encode())
                        last_ping = time.time()
                        ping_every = min(3600, max(60, int(res.get("next", 300))))
                    self.online = int(call("/count")["online"])
                except Exception:
                    self.online = None
            time.sleep(60)

    def _set_macro(self, on, why="F3"):
        """매크로 버튼 켜기 · 끄기 (F3 · 화면 버튼과 같음)"""
        with self.lock:
            self.data["macro_on"] = bool(on)
        self._save()
        if not on:
            self.mpop.stop()
            self.fisher.stop()
            self.items.stop()
            self.merchant.stop()
        self._on_log(f"{why} — 매크로 {'켜짐' if on else '꺼짐'}", "y" if not on else "g")

    def _hotkey_loop(self):
        """F3: 매크로 켜기 · 끄기 (어느 창에 있든) · 매크로 작동 중엔 화면 위에 '건드리지 마세요' 띠
        (이동 · 자동 보정 띠가 떠 있을 땐 그 띠만)"""
        was, banner, tick = False, None, 0
        while True:
            time.sleep(0.05)
            try:
                down = macro.key_down_now("f3")
                if down and not was:
                    self._set_macro(not self.data.get("macro_on"))
                was = down
                tick += 1
                if tick % 4:
                    continue
                other = (getattr(self, "_mv_banner", None) is not None and self._mv_banner.poll() is None) \
                    or getattr(self, "_autocal_running", False) or getattr(self, "_sellcal_running", False)
                want = bool(self.data.get("macro_on")) and (self.fisher.running() or self.mpop.running() or self.items.running()
                                                          or self.merchant.running()) and not other
                alive = banner is not None and banner.poll() is None
                if want and not alive:
                    banner = self._banner_proc("macro")
                elif not want and alive:
                    banner.kill()
                    banner = None
            except Exception as e:
                write_crash(f"hotkey loop: {e}")
                time.sleep(1)

    def _macro_user_stop(self):
        """F7: 매크로 버튼을 끔 (자동 낚시 · 매크로 탭 기능 전부 멈춤)"""
        with self.lock:
            self.data["macro_on"] = False
        self._save()
        self._on_log("F7 — 매크로 꺼짐", "y")

    def _on_biome_change(self, prev, found, sniping):
        """매크로 탭 · 레어 바이옴 자동 팝핑: 지금 켜져 있는 로블록스(내 서버)에서 레어 바이옴이 시작되면 포션 사용
        스나이핑으로 들어간 다른 사람 서버이거나, 오토 팝핑 · 복귀 · Play 클릭이 도는 중이면 안 함"""
        mp = self.data.get("mpop", {})
        if not self.data.get("macro_on") or not mp.get("enabled") or found not in popping.POP_BIOMES:
            return
        if sniping:
            self._on_log(f"{found} 감지 — 스나이핑 접속이라 내 서버 팝핑 안 함", "d")
            return
        if self.pop.running() or self.ret.running() or self.play.running():
            self._on_log(f"{found} 감지 — 다른 매크로가 도는 중이라 내 서버 팝핑 안 함", "y")
            return
        if not (mp.get("biomes_on") or {}).get(found, True):
            self._on_log(f"{found} 감지 — 매크로 탭 팝핑 바이옴에서 꺼져 있음", "y")
            return
        if not macro.roblox_window_cached(1.0):
            return                          # 옛 로그 파일 (로블록스가 꺼져 있음)
        # 레어 바이옴이 먼저 — 아이템 사용 · 상인 구매가 화면을 쓰는 중이면 멈추고 끝난 뒤 팝핑 (같이 클릭하면 꼬임)
        others = [f for f in (self.items, self.merchant) if f.running()]
        if not others:
            self.mpop.start(found)
            return
        for f in others:
            f.stop()

        def later():
            try:
                for f in others:
                    if f.thread:
                        f.thread.join(8)
                self.mpop.start(found)
            finally:
                self.mpop_wait = False
        self.mpop_wait = True               # 기다리는 동안 매크로 루프가 다른 기능을 새로 시작하지 않게
        threading.Thread(target=later, daemon=True).start()

    def _on_pop_end(self, stopped):
        if not stopped:                     # 바이옴 종료·접속 끊김 등으로 끝남 → 복귀 (직접 멈춘 경우 제외)
            self.ret.start("오토 팝핑 종료")

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
        region = self._pop_cfg().get("ocr_region")
        if not region:
            return {"error": "OCR 영역 먼저 지정"}
        back = macro.foreground()
        try:
            text = macro.ocr_region(region, item=True)
        except ModuleNotFoundError:
            return {"error": "OCR 패키지가 설치되지 않음 (run.bat 으로 실행 필요)"}
        except Exception as e:
            return {"error": f"OCR 오류: {e}"}
        finally:
            macro.focus_back(back)
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
        miss = popping.Popper.missing(self._mpop_cfg())
        if miss:
            return {"error": "매크로 기준 위치 설정 필요: " + ", ".join(miss)}
        if not [i for i in ((self.data.get("mpop", {}).get("templates") or {}).get(b) or {}).get("items", [])
                if i.get("name")]:
            return {"error": "포션 목록이 비어 있음"}
        self.mpop.start(b, test=True)
        return {"ok": True}

    # 매크로 기준 위치 설정 — 버튼 위치 · 영역 (feat: base = 여러 기능이 같이 쓰는 기준 위치 / mfish = 자동 낚시만)
    MPOS_POINTS = {"base": dict(popping.POS_KEYS, chat_pos="채팅 버튼", collection_pos="도감 버튼", collection_close="도감 Exit", dialog_pos="대화창"),
                   "mfish": dict(fishing.POS_KEYS, **{k: n for k, n in sell.SELL_KEYS if k != "info_region"}),
                   "mmerch": dict(merchant.POS_KEYS)}
    MPOS_REGIONS = {"base": ("ocr_region", "notice_region"), "mfish": ("panel_region", "reel_region", "result_region", "bar_region", "info_region"),
                    "mmerch": ("chat_region", "item_region")}
    # 16:9 위치 템플릿 (로블록스 창 기준 비율) — 스나이프 탭 오토 팝핑 16:9 템플릿과 같은 값
    # (자동 낚시는 템플릿 대신 낚시 창 · 결과창 영역으로 안쪽 위치를 계산 → fishing.WINDOW_KEYS)
    MPOS_TEMPLATE = {
        "base": {"inventory_pos": [0.018, 0.474], "items_pos": [0.663, 0.312], "search_pos": [0.458, 0.34],
                 "item_pos": [0.443, 0.44], "amount_pos": [0.296, 0.534], "use_pos": [0.356, 0.535],
                 "ocr_region": [0.418, 0.399, 0.466, 0.484],
                 # 1080p 기준: 채팅 버튼 (112, 30) · 도감 버튼 (47, 467) · 도감 Exit (382, 126) — FishSol 에서 쓰는 자리
                 "chat_pos": [0.0582, 0.0278], "collection_pos": [0.0245, 0.4324], "collection_close": [0.199, 0.1167],
                 "dialog_pos": [0.3979, 0.763]},     # NPC 대화창 (Noteab 매크로 1080p 프리셋 · Apache 2.0)
        # 판매 (Noteab 매크로의 1920x1080 위치 프리셋 · Apache 2.0) — Sell Fish 버튼 · 물고기 정보 영역은 직접 지정
        "mfish": {"first_fish_pos": [0.4349, 0.3778], "sell_all_pos": [0.3464, 0.7444],
                  "confirm_sell_pos": [0.4141, 0.5731], "shop_close_pos": [0.7609, 0.2528]},
        # 상인 — 1920x1080 전체 화면 스크린샷에서 잰 값 (채팅 위치는 Noteab 매크로 1080p 프리셋 · Apache 2.0)
        "mmerch": {"chat_hover": [0.0365, 0.1778], "chat_region": [0.0042, 0.0935, 0.251, 0.3426],
                   "open_pos": [0.3396, 0.8759], "first_slot": [0.5021, 0.6667], "second_slot": [0.601, 0.6667],
                   "item_region": [0.5724, 0.3444, 0.9427, 0.3741], "max_pos": [0.6984, 0.5685],
                   "purchase_pos": [0.6125, 0.613], "close_pos": [0.9422, 0.3213]},
    }

    BANNER_PROGRESS = Path(tempfile.gettempdir()) / f"acrux_banner_{os.getpid()}.txt"

    def _banner_progress(self, to, sec=0.3):
        """안내 띠 게이지: 지금 값에서 to(0~1) 까지 sec 초 동안 차오름"""
        try:
            tmp = self.BANNER_PROGRESS.with_suffix(".tmp")
            tmp.write_text(f"{min(1.0, max(0.0, to)):.4f} {max(0.0, sec):.2f}", encoding="utf-8")
            os.replace(tmp, self.BANNER_PROGRESS)
        except OSError:
            pass

    def _banner_proc(self, kind):
        """화면 위 가운데 '건드리지 마세요' 안내 띠 (별도 프로세스 · 회색 → 파란 게이지) → Popen 또는 None
        시간 재기 띠는 끝을 알 수 없어서 다 찬 상태로 · 매크로 작동 띠는 게이지 없이 (앱이 꺼지면 같이 닫힘)"""
        env = dict(os.environ, ACRUX_LANG=str(self.data.get("lang") or "ko"))
        if kind == "macro":
            args = ["--banner", kind, "--parent", str(os.getpid())]
        else:
            self._banner_progress(1.0 if kind == "measure" else 0.0, 0)
            args = ["--banner", kind, "--progress", str(self.BANNER_PROGRESS)]
        cmd = [sys.executable, *args] if getattr(sys, "frozen", False) else \
            [sys.executable, str(Path(__file__).resolve().parent / "macro.py"), *args]
        try:
            return subprocess.Popen(cmd, creationflags=NO_WINDOW, env=env)
        except Exception:
            return None

    def _move_banner(self, kind):
        """이동 중 안내 띠 바꾸기 (kind: 'move' · 'measure' · None = 끔)"""
        old = getattr(self, "_mv_banner", None)
        if old is not None and old.poll() is None:
            old.kill()
        self._mv_banner = self._banner_proc(kind) if kind else None

    def api_mfish_autocal(self, _):
        """자동 보정 (전부 자동): 화면 위에 '건드리지 마세요' 띠를 띄우고
        ① 대기 창(Fish 버튼) 테두리 → ② Fish 를 직접 눌러 입질 → 미니게임 창 테두리 → ③ (릴링은 안 함) 결과창 테두리 → X
        를 차례로 재서 위치를 전부 맞춘 뒤 띠를 끄고 Acrux 화면으로 돌아옴 · F7 이나 버튼을 한 번 더 누르면 취소
        자동 낚시가 돌고 있으면 잠깐 멈췄다가 끝나면 이어감"""
        if getattr(self, "_autocal_running", False):
            self._autocal_stop.set()                 # 진행 중에 다시 누르면 취소
            return {"error": "자동 보정 취소 중"}
        hwnd = macro.roblox_window_cached(1.0)
        if not hwnd:
            return {"error": "로블록스 창 없음"}
        self._autocal_running, self._autocal_stop = True, threading.Event()
        back = macro.foreground()
        banner = None
        try:
            if not self.fisher.hold(45):             # 자동 낚시 중이면 안전한 곳(Fish 버튼)에서 잠깐 멈춤
                return {"error": "자동 낚시가 멈추지 않음 — 낚시를 끄고 다시 눌러주세요"}
            banner = self._banner_proc("autocal")
            result = self._autocal_run(hwnd)
        except (_AutocalStop, fishing.Stopped) as e:
            result = {"error": str(e) or "자동 보정 취소됨"}
        except Exception as e:
            result = {"error": f"자동 보정 실패: {e}"}
        finally:
            if banner and banner.poll() is None:
                banner.kill()
            self.fisher.release()
            if not self.fisher.running():
                self.fisher._set(msg="대기")
            self._autocal_running = False
            macro.focus_back(back)                   # 끝나면 Acrux 화면으로
        if result.get("error") and self.data.get("mfish"):
            self._save()                             # 멈추기 전까지 잰 위치는 저장
            result["mfish"] = self.data["mfish"]
            self._on_log(result["error"], "y")
        return result

    def _autocal_run(self, hwnd):
        import numpy as np
        import mss

        def check():
            if self._autocal_stop.is_set() or macro.key_down_now("f7"):
                raise _AutocalStop("자동 보정 취소됨")

        def wait(sec):
            end = time.time() + sec
            while time.time() < end:
                check()
                time.sleep(0.05)

        def shot():
            rect = macro.client_rect(hwnd)
            if not rect:
                raise _AutocalStop("로블록스 창 없음")
            with mss.mss() as m:
                g = m.grab({"left": rect[0], "top": rect[1], "width": rect[2], "height": rect[3]})
            return rect, np.frombuffer(g.rgb, np.uint8).reshape(g.height, g.width, 3).astype(np.int16)

        def click(pos):
            rect = macro.client_rect(hwnd)
            macro.focus(hwnd)
            x, y = macro.to_screen(pos[0], pos[1], rect)
            macro.click(x, y)

        def frame_of(rgb, key):
            W, H = rgb.shape[1], rgb.shape[0]
            lo, hi = fishing.WINDOW_SHAPES[key]
            for (x1, y1, x2, y2) in fishing.find_frames(rgb):
                if lo <= (x2 - x1) / max(1, y2 - y1) <= hi:
                    return [round(x1 / W, 4), round(y1 / H, 4), round(x2 / W, 4), round(y2 / H, 4)]
            return None

        def settled(key, sec):
            """창이 열리는 애니메이션(올라옴 · 커짐) 중이면 크기가 다름 → 연속 두 화면에서 같은 자리일 때만 씀"""
            prev, end = None, time.time() + sec
            while time.time() < end:
                check()
                cur = frame_of(shot()[1], key)
                if cur and prev and all(abs(p - q) <= 0.002 for p, q in zip(cur, prev)):
                    return cur
                prev = cur
                time.sleep(0.1)
            return None

        def apply(key, region):
            with self.lock:
                c = self.data.setdefault("mfish", {})
                c[key] = region
                c.update(fishing.layout_from(region, fishing.WINDOW_KEYS[key]))

        macro.focus(hwnd, wait=0.4)
        done = []
        prog = self._banner_progress
        prog(0.15, 1.0)
        # ① 대기 창 (Fish 버튼이 보여야 함)
        rect, rgb = shot()
        panel = frame_of(rgb, "panel_region")
        if not panel:
            raise _AutocalStop("낚시 창을 못 찾음 — 낚시 자리에서 Fish 버튼이 보일 때 눌러주세요")
        apply("panel_region", panel)
        x1, y1, x2, y2 = panel                       # 미니게임 창은 우선 대기 창으로 계산 (아래에서 실제로 재면 바뀜)
        c0, half = (x1 + x2) / 2, (x2 - x1) / 2 * fishing.REEL_FROM_PANEL
        apply("reel_region", [round(c0 - half, 4), y1, round(c0 + half, 4), y2])
        done.append("대기 창")
        prog(0.2, 0.3)
        mf = self.data["mfish"]
        # ② Fish 를 눌러 입질 → 미니게임 창
        with macro.ScreenGrabber() as sct:
            def biting():
                rect = macro.client_rect(hwnd)
                return self.fisher._bar_seen(sct, rect, mf) or self.fisher._diamond_moved(sct, rect, mf)

            def cast():                              # Fish → Exit 로 바뀜 (또는 그새 입질이 옴)
                end = time.time() + 1.5
                while time.time() < end:
                    check()
                    if fishing.button_state(self.fisher._grab_box(sct, macro.client_rect(hwnd), mf["fish_btn"])) == "exit" \
                            or biting():
                        return True
                    time.sleep(0.1)
                return False
            for _ in range(3):                       # 최대 3번 눌러 봄
                click(mf["fish_btn"])
                if cast():
                    break
            else:
                raise _AutocalStop("Fish 를 눌러도 반응 없음 (인벤토리 가득?) — 대기 창만 맞춤")
            prog(0.55, 25)                           # 입질은 언제 올지 몰라서 천천히
            end = time.time() + float(mf.get("bite_max", 60)) + 10
            while time.time() < end:                 # 입질 기다림 (미니게임 창이 뜰 때까지)
                check()
                if biting():
                    break
                time.sleep(0.1)
            else:
                click(mf["fish_btn"])                # 던진 걸 거둠 (Exit)
                raise _AutocalStop("입질이 안 와서 미니게임 창은 못 잼 — 대기 창만 맞춤")
            prog(0.6, 0.3)
            reel = settled("reel_region", 3.0)       # 미니게임 창
            if reel:
                apply("reel_region", reel)
                done.append("미니게임 창")
            else:
                done.append("미니게임 창 (대기 창으로 계산)")
            # ③ 방금 잰 위치로 릴링해서 물고기를 잡음 → 결과창이 뜸
            cfg = dict(fishing.DEFAULTS, **self.data["mfish"])
            prog(0.85, 12)
            self.fisher._reel(sct, macro.client_rect(hwnd), cfg, self._autocal_stop)   # 취소 · F7 이면 Stopped
        mf = self.data["mfish"]
        prog(0.9, 0.3)
        result = settled("result_region", 6.0)
        if result:
            apply("result_region", result)
            done.append("결과창")
        mf = self.data["mfish"]
        # 결과창 닫기 (Fish 버튼이 다시 보일 때까지 X)
        if mf.get("close_pos"):
            with macro.ScreenGrabber() as sct:
                for _ in range(30):
                    check()
                    if fishing.button_state(self.fisher._grab_box(sct, macro.client_rect(hwnd), mf["fish_btn"])) == "fish":
                        break
                    click(mf["close_pos"])
                    wait(0.2)
        self._save()
        prog(1.0, 0.2)
        text = " · ".join(done)
        self._on_log(f"자동 보정 완료: {text}", "g")
        return {"mfish": self.data["mfish"], "done": text}

    # ---------------- 이동 (통합 위치 → 이동) ----------------
    def _set_move_time(self, i, j, sec):
        with self.lock:
            try:
                self.data["move"]["places"][i]["points"][j]["time"] = sec
            except (KeyError, IndexError, TypeError):
                return
        self._save()

    def _move_point(self, p):
        places = self.data.get("move", {}).get("places") or []
        i, j = int(p.get("place", -1)), int(p.get("point", -1))
        if not 0 <= i < len(places) or not 0 <= j < len(places[i].get("points") or []):
            return None, None
        return i, j

    # ---------------- 판매 (자동 낚시 → 판매) ----------------
    def _on_fish_start(self, stop):
        """자동 낚시 스레드에서 (낚시를 시작할 때마다): 기준 장소 → 낚시 장소로 이동
        낚시 장소가 아직 설정 안 됐거나 이동이 실패하면 알림만 띄우고 지금 자리에서 낚시"""
        m = self.seller.fish_missing()
        if m:
            self._on_log(f"낚시 장소로 이동 안 함 — {m} · 지금 자리에서 낚시", "n")
            return
        try:
            self.seller.go_fish(stop)
        except move.Stopped:
            if not stop.is_set():                  # 자동 낚시가 멈춘 게 아니면 F7 · 멈춤 버튼 → 매크로 끔
                self._macro_user_stop()
            raise fishing.Stopped()
        except Exception as e:
            self._on_log(f"낚시 장소로 이동 실패: {e} · 지금 자리에서 낚시", "n")

    def _on_fish_full(self, stop):
        """자동 낚시 스레드에서: 인벤토리 가득 → 판매 · 돌아오면 True (낚시 이어감) / 못 하면 False (낚시 멈춤)"""
        m = self.seller.missing()
        if m:
            self._on_log(f"인벤토리 가득 — 판매 못 함: {m}", "n")
            return False
        try:
            self.seller.run(stop)
            return True
        except move.Stopped:
            if not stop.is_set():                  # 자동 낚시가 멈춘 게 아니면 F7 · 멈춤 버튼 → 매크로 끔
                self._macro_user_stop()
            raise fishing.Stopped()
        except Exception as e:
            self._on_log(f"판매 실패: {e}", "n")
            return False

    def api_sell_autocal(self, _):
        """판매 자동 보정: 플레이어가 Captain Flarg 앞에서 E 를 누르면 대화창 → [Sell Fish] → 상점을 글자(OCR)로 찾아
        대화창(통합 위치) · Sell Fish · 첫 칸 · Sell All · 확인 Sell · 상점 X · 물고기 정보 영역을 맞춤 (실제로 팔지는 않음)
        F7 이나 버튼을 한 번 더 누르면 취소 · 자동 낚시가 돌고 있으면 잠깐 멈췄다가 이어감"""
        if getattr(self, "_sellcal_running", False):
            self._sellcal_stop.set()
            return {"error": "판매 자동 보정 취소 중"}
        if self.mover.running():
            return {"error": "이동이 도는 중"}
        hwnd = macro.roblox_window_cached(1.0)
        if not hwnd:
            return {"error": "로블록스 창 없음"}
        self._sellcal_running, self._sellcal_stop = True, threading.Event()
        stop, back, found, banner = self._sellcal_stop, macro.foreground(), {}, [None]

        def set_banner(kind):
            if banner[0] is not None and banner[0].poll() is None:
                banner[0].kill()
            banner[0] = self._banner_proc(kind) if kind else None

        def wait(sec):
            end = time.time() + sec
            while True:
                if stop.is_set() or macro.key_down_now("f7"):
                    raise _AutocalStop("판매 자동 보정 취소됨")
                if time.time() >= end:
                    return
                time.sleep(0.03)

        def click(pos):
            h = macro.roblox_window_cached(1.0)
            rect = macro.client_rect(h) if h else None
            if not rect:
                raise _AutocalStop("로블록스 창 없음")
            macro.focus(h)
            macro.click(*macro.to_screen(pos[0], pos[1], rect))

        def aspect():
            rect = macro.client_rect(macro.roblox_window_cached(1.0))
            return rect[2] / max(1, rect[3])

        def status(msg):
            self._on_log(f"판매 자동 보정 · {msg}", "c")
            if msg.startswith("대화창 찾음"):
                set_banner("autocal")               # 이제부턴 매크로가 누름 → '건드리지 마세요'
                self._banner_progress(0.2, 2.0)
            step = {"상점 여는 중": (0.5, 4.0), "확인창 확인 중 (팔지는 않음)": (0.8, 3.0), "상점 닫는 중": (0.95, 1.0)}.get(msg)
            if step:
                self._banner_progress(*step)

        try:
            if not self.fisher.hold(45):
                return {"error": "자동 낚시가 멈추지 않음 — 낚시를 끄고 다시 눌러주세요"}
            set_banner("sellcal")
            macro.focus(hwnd, wait=0.3)
            notes = sell.autocal(lambda r: macro.ocr_boxes(r), click, wait, aspect, found, status)
            error = None
        except _AutocalStop as e:
            error, notes = str(e), []
        except Exception as e:
            error, notes = f"판매 자동 보정 실패: {e}", []
        finally:
            set_banner(None)
            self.fisher.release()
            self._sellcal_running = False
            macro.focus_back(back)
        if found:                                    # 멈추기 전까지 찾은 위치는 저장
            with self.lock:
                b, mf = self.data.setdefault("base", {}), self.data.setdefault("mfish", {})
                for k, v in found.items():
                    (b if k == "dialog_pos" else mf)[k] = v
            self._save()
        names = dict(self.MPOS_POINTS["base"], **self.MPOS_POINTS["mfish"], info_region="물고기 정보 영역")
        done = ", ".join(names.get(k, k) for k in found)
        res = {"base": self.data.get("base"), "mfish": self.data.get("mfish"), "found": list(found)}
        if error:
            self._on_log(error + (f" (찾은 것: {done})" if done else ""), "n")
            res["error"] = error
        else:
            self._on_log(f"판매 자동 보정 완료: {done}" + (" · " + " · ".join(notes) if notes else ""), "g")
            res["done"] = done
            res["notes"] = notes
        return res

    def api_sell_test(self, _):
        """판매 테스트: 기준 장소 → 물고기 판매 장소 → 판매 → 낚시 장소 (자동 낚시는 잠깐 멈췄다가 이어감)"""
        if self.mover.running():
            return {"error": "이동이 이미 도는 중"}
        m = self.seller.missing()
        if m:
            return {"error": m}

        def run():
            try:
                if not self.fisher.hold(45):
                    self._on_log("자동 낚시가 멈추지 않아 판매 테스트를 못 함", "n")
                    return
                self.seller.run()
            except move.Stopped:
                self._on_log("판매 멈춤", "y")
            except Exception as e:
                self._on_log(f"판매 실패: {e}", "n")
            finally:
                self.fisher.release()
        threading.Thread(target=run, daemon=True).start()
        return {"ok": True}

    def api_move_base(self, _):
        miss = self.mover.base_missing()
        if miss:
            return {"error": miss}
        if not self.mover.start("base"):
            return {"error": "이동이 이미 도는 중"}
        return {"ok": True}

    def api_move_place(self, p):
        i = int(p.get("place", -1))
        miss = self.mover.base_missing() or self.mover.path_missing(i)
        if miss:
            return {"error": miss}
        if not self.mover.start("place", i):
            return {"error": "이동이 이미 도는 중"}
        return {"ok": True}

    def api_move_test(self, p):
        """지점까지 걸리는 시간 재기 — from_base: 기준 장소부터 (아니면 지금 자리에서 바로)"""
        i, j = self._move_point(p)
        if i is None:
            return {"error": "지점을 찾을 수 없음"}
        if not self.data["move"]["places"][i]["points"][j].get("pos"):
            return {"error": "지점 위치를 먼저 지정"}
        if p.get("from_base", True):
            miss = self.mover.base_missing() or self.mover.path_missing(i, upto=j)
            if miss:
                return {"error": miss}
        if not self.mover.start("test", i, j, bool(p.get("from_base", True))):
            return {"error": "이동이 이미 도는 중"}
        return {"ok": True}

    def api_move_get(self, _):
        return {"move": self.data.get("move", {})}

    def api_move_arrive(self, _):
        self.mover.arrive()
        return {"ok": True}

    def api_move_stop(self, _):
        self.mover.stop()
        return {"ok": True}

    def api_move_pick(self, p):
        """지점 위치 지정 — from_base: 매크로가 기준 장소(+ 앞 지점들)까지 간 뒤 / 아니면 지금 화면에서 바로
        로블록스 화면에서 걸어갈 곳을 클릭 → 저장 (위치가 바뀌면 잰 시간은 지움)"""
        i, j = self._move_point(p)
        if i is None:
            return {"error": "지점을 찾을 수 없음"}
        if p.get("from_base"):
            miss = self.mover.base_missing() or self.mover.path_missing(i, upto=j)
            if miss:
                return {"error": miss}
            if not self.mover.start("prep", i, j):
                return {"error": "이동이 이미 도는 중"}
            self.mover.thread.join(180)
            err = self.mover.snapshot().get("error")
            if self.mover.running() or err or self.mover.stop_ev.is_set():
                return {"error": err or "기준 장소로 못 감 (멈춤)"}
        r = self._pick_overlay("--pick-point")
        if r.get("error"):
            return r
        with self.lock:
            pt = self.data["move"]["places"][i]["points"][j]
            pt["pos"], pt["time"] = [round(r["x"], 4), round(r["y"], 4)], None
        self._save()
        return {"move": self.data["move"]}

    def api_mpos_point(self, p):
        feat, key = str(p.get("feat", "")), str(p.get("key", ""))
        if key not in self.MPOS_POINTS.get(feat, {}):
            return {"error": "알 수 없는 항목"}
        r = self._pick_overlay("--pick-point")
        if r.get("error"):
            return r
        with self.lock:
            self.data.setdefault(feat, {})[key] = [round(r["x"], 4), round(r["y"], 4)]
        self._save()
        return {"pos": self.data[feat][key]}

    def api_mpos_region(self, p):
        """영역 드래그 — mfish 의 낚시 창 · 결과창 영역은 안쪽 위치(Fish 버튼 · 릴링 바 · 결과창 X · 제목)까지 계산해서 저장"""
        feat = str(p.get("feat", ""))
        keys = self.MPOS_REGIONS.get(feat, ())
        key = str(p.get("key") or (keys[0] if keys else ""))
        if key not in keys:
            return {"error": "알 수 없는 항목"}
        r = self._pick_overlay("--pick-region")
        if r.get("error"):
            return r
        with self.lock:
            c = self.data.setdefault(feat, {})
            c[key] = r["region"]
            if feat == "mfish" and key in fishing.WINDOW_KEYS:
                lay = fishing.layout_from(r["region"], fishing.WINDOW_KEYS[key])
                if key == "panel_region" and c.get("reel_region"):
                    lay.pop("bar_region", None)       # 미니게임 창을 따로 지정했으면 릴링 바는 그쪽 기준 그대로
                c.update(lay)
        self._save()
        return {"region": r["region"], feat: self.data[feat]}

    # 화면 비율 — 로블록스 UI 는 화면 높이에 맞춰 커지고, 낚시 창 · 결과창 · 인벤토리 창은 가로 가운데 기준,
    # Inventory 버튼(왼쪽 메뉴)은 왼쪽 끝 기준이라고 보고 16:9 값을 바꿈 (16:9 가 아닌 비율은 추정값)
    MPOS_RATIOS = {"16:9": 16 / 9}           # 다른 비율은 추정값이라 불안정해서 뺌
    MPOS_LEFT = {"inventory_pos", "chat_pos", "collection_pos", "collection_close", "chat_hover", "chat_region"}

    @classmethod
    def _mpos_scaled(cls, feat, aspect):
        k = (16 / 9) / aspect

        def fx(key, x):
            v = x * k if key in cls.MPOS_LEFT else 0.5 + (x - 0.5) * k
            return round(min(1.0, max(0.0, v)), 4)
        out = {}
        for key, v in cls.MPOS_TEMPLATE[feat].items():
            out[key] = ([fx(key, v[0]), v[1], fx(key, v[2]), v[3]] if len(v) == 4 else [fx(key, v[0]), v[1]])
        return out

    def api_mpos_template(self, p):
        feat = str(p.get("feat", ""))
        if feat not in self.MPOS_TEMPLATE:
            return {"error": "알 수 없는 항목"}
        ratio = str(p.get("ratio") or "16:9")
        if ratio in self.MPOS_RATIOS:
            aspect, label = self.MPOS_RATIOS[ratio], ratio
        else:
            return {"error": "알 수 없는 화면 비율"}
        t = self._mpos_scaled(feat, aspect)
        with self.lock:
            self.data.setdefault(feat, {}).update(t)
        self._save()
        return {feat: self.data[feat], "label": label, "guess": abs(aspect - 16 / 9) > 0.02}

    def api_mpos_copy_pop(self, _):
        """기준 위치(인벤토리) ← 스나이프 탭 오토 팝핑에 지정한 위치 그대로 복사"""
        src = self.data.get("pop", {})
        keys = (*self.MPOS_POINTS["base"], "ocr_region")
        if not any(src.get(k) for k in keys):
            return {"error": "스나이프 탭 오토 팝핑에 지정된 위치 없음"}
        with self.lock:
            b = self.data.setdefault("base", {})
            for k in keys:
                if src.get(k):
                    b[k] = json.loads(json.dumps(src[k]))
        self._save()
        return {"base": self.data["base"]}

    def api_mpop_ocr_test(self, _):
        region = self.data.get("base", {}).get("ocr_region")
        if not region:
            return {"error": "OCR 영역 먼저 지정"}
        back = macro.foreground()
        try:
            text = macro.ocr_region(region, item=True)
        except ModuleNotFoundError:
            return {"error": "OCR 패키지가 설치되지 않음 (run.bat 으로 실행 필요)"}
        except Exception as e:
            return {"error": f"OCR 오류: {e}"}
        finally:
            macro.focus_back(back)
        name, count = popping.parse_ocr(text)
        self._on_log(f"OCR 테스트: '{text[:60]}' → 이름 '{name}' · 개수 {count}", "d")
        return {"text": text, "name": name, "count": count, "engine": macro.ocr_engine_name()}

    def api_mfish_check(self, _):
        """상태 확인: 지금 화면에서 Fish 버튼 색 · 릴링 바 · 결과창 제목을 읽어서 알려줌"""
        mf = self._mfish_cfg()
        hwnd = macro.roblox_window_cached(1.0)
        if not hwnd:
            return {"error": "로블록스 창 없음"}
        # Acrux 창이 로블록스를 가리면 Acrux 화면을 읽게 됨 → 로블록스를 맨 앞으로 띄우고 확인 후 Acrux 로 돌아옴
        back = macro.foreground()
        macro.focus(hwnd, wait=0.35)
        rect = macro.client_rect(hwnd)
        if not rect:
            return {"error": "로블록스 창 없음"}
        try:
            out = {}
            with macro.ScreenGrabber() as sct:
                if mf.get("fish_btn"):
                    st = fishing.button_state(self.fisher._grab_box(sct, rect, mf["fish_btn"]))
                    out["button"] = {"fish": "Fish (파랑)", "exit": "Exit (빨강)"}.get(st, "안 보임")
                if mf.get("bar_region"):
                    box, top_in, bh = self.fisher._bar_geom(rect, mf["bar_region"])
                    xs = box.pop("_x")
                    img = sct.grab(box)
                    rgb = fishing._np(bytes(img.bgra), img.width, img.height)
                    found = fishing.find_bar_rows(rgb, top_in, bh)
                    if found:
                        top_in, bh, xs = found[0], found[1], (found[2] + 1, found[3])
                    a = fishing.analyze_bar(rgb[:, xs[0]:xs[1]], top_in, bh)
                    out["bar"] = (f"보임 · 내 위치 {a['marker']:.0f} · 구간 {a['zone']}" if a["present"] and a["marker"] is not None
                                  else "보임" if a["present"] else "안 보임")
                if mf.get("panel_region"):
                    moved = self.fisher._diamond_moved(sct, rect, mf)
                    out["diamond"] = "미니게임 자리" if moved else "대기 자리 (미니게임 아님)"
                if mf.get("notice_region"):
                    text = macro.notice_check(sct, rect, mf["notice_region"], fishing.FULL_WORDS)
                    out["notice"] = "인벤토리 가득 알림 있음" if text else "없음"
                if mf.get("title_pos"):
                    t = fishing.classify_title(self.fisher._grab_box(sct, rect, mf["title_pos"], 0.12, 0.05))
                    out["title"] = {"success": "성공", "junk": "쓰레기", "fail": "실패"}.get(t, "안 보임")
            return out
        except Exception as e:
            return {"error": f"확인 실패: {e}"}
        finally:
            macro.focus_back(back)

    def api_mfish_stop(self, _):
        self.fisher.stop()
        return {"ok": True}

    def api_mpop_stop(self, _):
        self.mpop.stop()
        return {"ok": True}

    def api_mitem_test(self, _):
        """오토 아이템 사용 테스트: 켜 둔 아이템을 쿨타임과 상관없이 지금 한 번 사용"""
        miss = popping.Popper.missing(self.items._merged())
        if miss:
            return {"error": "매크로 기준 위치 설정 필요: " + ", ".join(miss)}
        keys = [k for k, _, _ in popping.ItemUser.ITEMS if (self.data.get("mitem") or {}).get(k)]
        if not keys:
            return {"error": "사용할 아이템이 꺼져 있음"}
        if self.items.running() or self.mover.running():
            return {"error": "다른 동작이 도는 중"}
        self.items.start(keys, test=True)
        return {"ok": True}

    def _on_merchant_done(self, job, name):
        if job == "check" and name:
            if (self.data.get("mmerch") or {}).get("buy") and any(
                    k.startswith(name + "_") for k in self.data["mmerch"]["buy"]):
                self.merchant_pending = (name, time.time())
            else:
                self._on_log(f"{name} — 살 아이템이 없어서 안 감", "d")

    def _on_merchant_cal(self, lay):
        with self.lock:
            self.data.setdefault("mmerch", {}).update(lay)
        self._save()

    def api_mmerch_check(self, _):
        """채팅 확인 테스트: 지금 채팅창을 한 번 읽어 봄 (상인이 있으면 구매까지 이어감)"""
        c = self.data.get("mmerch") or {}
        if not c.get("chat_region"):
            return {"error": "채팅 글자 영역 지정 필요 (매크로 기준 위치 설정 → 상인)"}
        if self.merchant.running():
            return {"error": "상인 자동 구매가 도는 중"}
        self.merchant.seen.clear()
        self.merchant.start_job("check")
        return {"ok": True}

    def api_mmerch_buy_test(self, p):
        """구매 테스트: Merchant Teleporter 부터 끝까지 한 번 (상인이 와 있을 때)"""
        name = "Jester" if p.get("name") == "Jester" else "Mari"
        miss = merchant.Merchant.missing(self.data.get("mmerch") or {}, self.data.get("base") or {})
        if miss:
            return {"error": "위치 설정 필요: " + ", ".join(miss)}
        if self.merchant.running() or self.mover.running():
            return {"error": "다른 동작이 도는 중"}
        self.fisher.stop()
        self.merchant.start_job("buy", name)
        return {"ok": True}

    def api_mmerch_stop(self, _):
        self.merchant.stop()
        self.merchant_pending = None
        return {"ok": True}

    def api_mitem_stop(self, _):
        self.items.stop()
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
        --pick-region: 드래그로 영역 / --pick-point: 클릭 1번으로 위치 / --pick-file: 프로그램 고르기
        선택 창이 가끔 안 뜨고(로블록스 뒤에 숨는 등) 그대로 2분 넘게 기다리느라 다른 위치 지정도 막히던 문제 →
        창이 실제로 떴는지 신호(.shown 파일)를 받고, 6초 안에 안 뜨면 끄고 한 번 더 띄움 · 그래도 안 되면 바로 실패로 끝냄"""
        for attempt in range(2):
            r = self._pick_once(flag)
            if r.get("error") != "__not_shown__":
                return r
            if attempt == 0:
                self._on_log("위치 지정 창이 안 떠서 다시 띄움", "y")
        return {"error": "위치 지정 창이 안 뜸 — 로블록스를 창 모드로 두고 다시 시도하세요"}

    def _pick_once(self, flag):
        import tempfile
        self.api_pick_cancel(None)                 # 남아 있던 선택 창이 있으면 정리 (안 그러면 계속 막힘)
        out = Path(tempfile.gettempdir()) / f"acrux_pick_{os.getpid()}.json"
        shown = Path(str(out) + ".shown")
        out.unlink(missing_ok=True)
        shown.unlink(missing_ok=True)
        if getattr(sys, "frozen", False):
            cmd = [sys.executable, flag, str(out)]
        else:
            cmd = [sys.executable, str(Path(__file__).resolve().parent / "macro.py"), flag, str(out)]
        env = dict(os.environ, ACRUX_LANG=str(self.data.get("lang") or "ko"))   # 안내 글자 언어
        limit = 300 if flag == "--pick-file" else 150
        self.fisher.no_focus = True             # 선택 창이 떠 있는 동안 자동 낚시가 로블록스를 앞으로 끌어오지 않게
        try:
            proc = subprocess.Popen(cmd, creationflags=NO_WINDOW, env=env)
            self._pick_proc = proc
            start = time.time()
            while proc.poll() is None:
                waited = time.time() - start
                # 프로그램 고르기 창은 윈도우 기본 창이라 신호를 안 보냄 → 시간만 봄
                if flag != "--pick-file" and waited > 6 and not shown.exists():
                    proc.kill()
                    return {"error": "__not_shown__"}
                if waited > limit:
                    proc.kill()
                    return {"error": "시간 초과"}
                time.sleep(0.1)
            if getattr(self, "_pick_cancelled", False):
                return {"error": "취소됨"}
            if not out.exists():
                return {"error": "지정 실패"}
            return json.loads(out.read_text(encoding="utf-8"))
        except Exception as e:
            return {"error": str(e)}
        finally:
            self._pick_proc, self._pick_cancelled = None, False
            self.fisher.no_focus = False
            out.unlink(missing_ok=True)
            shown.unlink(missing_ok=True)

    def api_pick_cancel(self, _):
        """떠 있는(또는 안 보이게 멈춘) 선택 창 끄기 — 같은 버튼을 한 번 더 누르면 취소"""
        proc = getattr(self, "_pick_proc", None)
        if proc and proc.poll() is None:
            self._pick_cancelled = True
            try:
                proc.kill()
            except Exception:
                pass
        return {"ok": True}

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
        return miss + popping.Popper.missing(self._pop_cfg())

    def api_start(self, _):
        miss = self.pop_missing()
        if miss:                                  # 오토 팝핑 설정을 안 하면 매크로를 켜지 않음
            self._on_log(f"시작 안 됨 — 오토 팝핑 설정 필요: {', '.join(miss)}", "n")
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

    # 업데이트 로그 — GitHub 릴리스 설명(RELEASE_NOTES.md 로 올라간 것)을 그대로 가져옴 · 10분 동안은 다시 안 받음
    RELEASES_API = "https://api.github.com/repos/rngenesis0-coder/Acrux-app/releases?per_page=100"
    _changelog_cache = (0.0, None)

    @staticmethod
    def _split_notes(body):
        """릴리스 설명 → {"ko": 한국어 부분, "en": 영어 부분} (## … 업데이트 / ## … Update 제목 기준)"""
        parts, cur = {"ko": [], "en": []}, None
        for line in (body or "").replace("\r\n", "\n").split("\n"):
            h = re.match(r"\s*## .*?(업데이트|Update)\s*$", line)
            if h:
                cur = "ko" if h.group(1) == "업데이트" else "en"
            elif cur:
                parts[cur].append(line)
        return {k: "\n".join(v).strip() for k, v in parts.items()}

    def api_changelog(self, p):
        at, cached = self._changelog_cache
        if cached is not None and not p.get("refresh") and time.time() - at < 600:
            return cached
        try:
            import urllib.request
            req = urllib.request.Request(self.RELEASES_API, headers={
                "User-Agent": "AcruxMacro", "Accept": "application/vnd.github+json"})
            with urllib.request.urlopen(req, timeout=10) as r:
                rel = json.loads(r.read().decode("utf-8"))
        except Exception as e:
            return {"error": f"업데이트 로그를 못 불러옴: {e}"}
        entries = []
        for x in rel:
            if x.get("draft") or not str(x.get("tag_name", "")).startswith("v"):
                continue
            notes = self._split_notes(x.get("body"))
            entries.append({"version": x["tag_name"][1:], "date": str(x.get("published_at") or "")[:10], **notes})
        entries.sort(key=lambda e: [int(n) if n.isdigit() else 0 for n in e["version"].split(".")], reverse=True)
        out = {"entries": entries, "current": VERSION}
        Bridge._changelog_cache = (time.time(), out)
        return out

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
        # 화면은 이 PC 안의 페이지 하나뿐 → 업데이트 확인 · 동기화 · 미리 띄워 두는 여분 프로세스 등 쓸데없는 일은 끔 (메모리 절약)
        proc = subprocess.Popen([str(edge), f"--app={url}", f"--user-data-dir={DATA_DIR / 'ui'}",
                                 f"--window-size={WIN_W},{WIN_H}", "--no-first-run", "--no-default-browser-check",
                                 "--disable-features=Translate,msEdgeSidebarV2,SpareRendererForSitePerProcess",
                                 "--disable-extensions", "--disable-background-networking", "--disable-component-update",
                                 "--disable-sync", "--disable-default-apps", "--no-pings", "--disable-breakpad",
                                 "--disable-domain-reliability", "--js-flags=--optimize-for-size"])
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
    if "--banner" in sys.argv:               # 화면 위 안내 띠 (자동 보정 중 · 이동 중)
        _i = sys.argv.index("--banner")
        _p = sys.argv[sys.argv.index("--progress") + 1] if "--progress" in sys.argv else None
        _pp = sys.argv[sys.argv.index("--parent") + 1] if "--parent" in sys.argv else None
        macro.show_banner(sys.argv[_i + 1] if len(sys.argv) > _i + 1 else "autocal", progress=_p, parent=_pp,
                          seconds=86400 if _pp else 240)
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
