# -*- coding: utf-8 -*-
"""
Acrux macro — 창 화면은 HTML/CSS 로 그리고, 윈도우 기본 Edge 엔진(앱 모드)으로 띄운다.

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

import core
import mods as modmod
import macro
import biome
import rejoin
import popping
import fishing
import move
import sell
import merchant
import crafter
import memmatch
import steps as stepmod
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
        self.seq = 0
        self.logs = deque(maxlen=1000)       # (seq, time, msg, color)
        # 모드 · 플러그인: 이벤트 · 모드가 만든 API 이름 → 모드 ID · 모드가 쓰는 config.json 키 (mods.py)
        self.hooks = modmod.Hooks(self._on_log)
        self.mod_apis, self.mod_keys, self.mods = {}, set(), []
        self.crash = None              # 로블록스가 갑자기 꺼졌을 때: {"until": 복귀 실행 시각}
        self.status = ("idle", "대기 중")
        self.last_ping = 0.0

        core.Bus.on_log = self._on_log
        core.Bus.on_status = self._on_status
        # 바이옴 매크로: 로그는 항상 읽고(현재 바이옴 표시), 웹후크는 바이옴 매크로 토글이 켜져 있을 때만 (시작 버튼과 무관)
        self.biome = biome.BiomeWatcher(lambda: self.data.get("biome", {}),
                                        lambda: bool(self.data.get("biome", {}).get("enabled")),
                                        on_change=self._on_biome_change)
        # 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버)
        # 매크로 탭 · 자동 낚시 (제자리 낚시) — 매크로 버튼 + 켜기가 켜져 있는 동안 계속
        self.fisher = fishing.Fisher(self._mfish_cfg, self._on_log,
                                     on_user_stop=self._macro_user_stop)
        self.mpop = popping.MyServerPopper(lambda: self.data.get("pop", {}), self._mpop_cfg,
                                           self._on_log, before=lambda: self._fish_yield("mpop", 45),
                                           after=lambda: self.fisher.release("mpop"))
        # 매크로 탭 · 블러드 레인 자동 생성 (할로윈 이벤트): PUMPKIN MOON · GRAVEYARD 가 떠 있으면 Cursed Rune Fragment 1개 사용
        self.blood = popping.BloodRainer(lambda: self.data.get("pop", {}), self._mblood_cfg, self._on_log,
                                         before=lambda: self._fish_yield("blood", 45),
                                         after=lambda: self.fisher.release("blood"))
        self.blood_wait = False            # 블러드 레인이 다른 기능이 끝나길 기다리는 중
        self.blood_seen = None             # 마지막으로 처리한 PUMPKIN MOON · GRAVEYARD (바이옴 기록 한 줄) — 한 번 뜬 바이옴엔 한 번만
        self.urgent_lock = threading.Lock()   # 레어 바이옴 팝핑 · 블러드 레인이 같이 시작되지 않게
        # 이동: 기준 장소(리셋 · 카메라 정렬 · 줌) → 화면의 한 점을 눌러 장소로 걸어감 · 걸리는 시간은 직접 잼
        self.mover = move.Mover(lambda: self.data.get("base", {}), lambda: self.data.get("move", {}), self._on_log,
                                set_move_time=self._set_move_time, before=lambda: self._fish_yield("move", 45),
                                after=lambda: self.fisher.release("move"), banner=self._move_banner,
                                progress=self._banner_progress)
        # 판매: 자동 낚시 중 인벤토리가 가득 차면 물고기 판매 장소로 가서 팔고 낚시 장소로 돌아옴
        self.seller = sell.Seller(self.mover, self._mfish_cfg, lambda: self.data.get("move", {}), self._on_log)
        self.fisher.on_full = self._on_fish_full
        # 오토 아이템 사용: 쿨타임마다 Strange Controller · Biome Randomizer 사용 (자동 낚시는 잠깐 비켜줌)
        self.items = popping.ItemUser(lambda: self.data.get("pop", {}), lambda: self.data.get("base", {}),
                                      lambda: self.data.get("mitem", {}), self._on_log,
                                      before=lambda: self._fish_yield("items", 90, cancel=False),
                                      after=lambda: self.fisher.release("items"))
        # 상인 자동 구매: 일정 간격마다 채팅 확인 → 상인이 오면 Merchant Teleporter 로 가서 구매 (자동 낚시는 멈췄다가 다시 시작)
        self.merchant = merchant.Merchant(lambda: self.data.get("pop", {}), lambda: self.data.get("base", {}),
                                          lambda: self.data.get("mmerch", {}), self._on_log, on_done=self._on_merchant_done,
                                          on_cal=self._on_merchant_cal)
        self.merchant_pending, self.merchant_check_at = None, 0.0
        self.mpop_wait = False             # 레어 바이옴 팝핑이 다른 기능이 끝나길 기다리는 중
        # 포션 자동 제작: 알림에 'Auto Crafted' 가 뜨면 그 포션 이름으로 Stella 에게 가서 재료를 다시 채움 (자동 낚시는 멈췄다가 다시 시작)
        self.crafter = crafter.Crafter(lambda: self.data.get("mcraft", {}), self._on_log, self.seller._borrow,
                                       lambda: self._place_of("mcraft", "craft_spot"))
        self.craft_pending, self.craft_check_at, self.craft_seen, self.craft_warned = None, 0.0, {}, False
        # 오토 메모리 매치: 매크로를 켜면 메모리 매치 장소로 가서 확인 · 쿨타임이면 남은 시간 뒤에 다시 (자동 낚시는 멈췄다가 다시 시작)
        self.matcher = memmatch.Matcher(lambda: self.data.get("mmatch", {}), self._on_log, self.seller._borrow,
                                        lambda: self._place_of("mmatch", "match_spot"))
        # 자동 낚시가 계속 헛돌면(낚시 자리를 못 찾음 · 팔 물고기가 없는데 가득 참) 멈춰 둠 — F3 으로 매크로를 다시 켜면 풀림
        self.fish_block, self.fish_fails, self.zero_sells = None, 0, 0
        self.fish_retry_at, self.fish_backoff = 0.0, 0     # 막혔을 때 다시 시도할 시각 · 쉬는 시간 (초)
        self.fisher.on_start = self._on_fish_start
        self.play = rejoin.PlayClicker(lambda: self.data.get("play", {}), self._on_log,
                                       on_ingame=self._on_ingame, on_fail=self._on_play_fail)
        # 매크로 복귀 (로블록스가 꺼졌을 때 복구 · 모드가 다른 서버에 들어갔다 돌아올 때): 로블록스 전부 종료 → 1초 → 내 브섭 링크 → Play
        # 모드 일이 도는 동안은 매크로(자동 낚시 등)가 쉬고, 내 서버로 돌아오면 입장 후 대기 뒤 다시 켜짐
        self.resume_at = 0.0                # 이 시각 전엔 매크로를 다시 켜지 않음 (복귀 직후 게임이 다 뜰 때까지)
        self.ret = rejoin.Returner(self._ret_cfg, self._on_log, self.play,
                                   kill=core.kill_roblox, launch=core.open_link)
        self.ret_fails, self.ret_watch = 0, None   # 복귀 Play 실패 횟수 · 복귀 실패 뒤 직접 들어갔는지 보는 로그 감시
        self.ret_cause = None               # "crash": 로블록스가 꺼져서 하는 복구 (끝나면 하던 대로 · 커스텀 매크로는 안 돌림)
        self.crash_warned = None            # 자동 복구를 못 하는 이유를 한 번만 알림
        self.afk_at = time.time()           # AFK 방지 점프: 마지막으로 점프한 시각
        self.craft_busy = False             # Auto Crafted 알림 확인(OCR)이 도는 중
        # 통계 (Acrux 탭): 이번 실행은 기능마다 센 값 · 올타임은 그만큼씩 더해서 설정 파일에 (바뀐 뒤 1분마다 · 끌 때)
        self.started = time.time()
        self.stat_seen, self.stat_dirty = {}, 0.0
        macro.RESET_WHEN_STOPPED[0] = lambda: (self.mpop_wait or self.blood_wait) and bool(self.data.get("macro_on"))
        self.biome.start()                  # 바이옴 변경 콜백이 위 실행기들을 쓰므로 맨 마지막에 시작
        threading.Thread(target=self._macro_loop, daemon=True).start()
        threading.Thread(target=self._hotkey_loop, daemon=True).start()
        threading.Thread(target=self._afk_loop, daemon=True).start()
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

    def _mod_busy(self, kind):
        """모드 일이 도는 중 (mods.Hooks 의 busy)"""
        return self.hooks.any("busy", kind)

    def _mod_active(self):
        return self.hooks.any("active")

    # ---------------- API ----------------
    # ---- 기능별 설정 + 매크로 기준 위치 (여러 기능이 같이 쓰는 위치는 base 에 하나만 저장)
    def _mpop_cfg(self):
        """레어 바이옴 자동 팝핑 (내 서버) 설정 + 기준 위치의 인벤토리 위치 · OCR 영역"""
        b = self.data.get("base", {})
        return dict(self.data.get("mpop", {}), **{k: b.get(k) for k in (*core.BASE_INV_KEYS, "ocr_region")})

    def _mblood_cfg(self):
        """블러드 레인 자동 생성 설정 + 기준 위치의 인벤토리 위치 · OCR 영역"""
        b = self.data.get("base", {})
        return dict(self.data.get("mblood", {}), **{k: b.get(k) for k in (*core.BASE_INV_KEYS, "ocr_region")})

    def _mfish_cfg(self):
        """자동 낚시 설정 + 공통 위치의 알림 영역 · 대화창"""
        b = self.data.get("base", {})
        return dict(self.data.get("mfish", {}), notice_region=b.get("notice_region"), dialog_pos=b.get("dialog_pos"))

    def api_state(self, _):
        with self.lock:
            out = {"config": self.data, "status": list(self.status), "version": VERSION}
        out["mods"] = modmod.listing(self)
        self.hooks.emit("state", out)
        return out

    # ---------------- 로블록스가 갑자기 꺼짐 (매크로 작동 중) ----------------
    # 매크로(F3)가 켜져 있고, 매크로가 일부러 끈 게 아닌데 로블록스 창이 사라지면:
    #   오른쪽 아래 알림 + 소리 → 10초 뒤 자동 복구(매크로 복귀). 그 전에 [대기] 를 누르면 일부러 끈 걸로 보고 복귀 안 함
    #   복구가 끝나면 하던 대로 (매크로가 켜져 있으면 다시 시작)
    CRASH_WAIT = 10.0

    def _ret_cfg(self):
        """복귀 설정 — 내 브섭 링크가 비어 있으면 바이옴 매크로 설정의 브섭 링크를 씀"""
        r = dict(self.data.get("ret") or {})
        if not (r.get("ps_link") or "").strip():
            r["ps_link"] = ((self.data.get("biome") or {}).get("ps_link") or "").strip()
        return r

    def _recover_missing(self):
        """자동 복구에 필요한데 비어 있는 설정"""
        pl = self.data.get("play") or {}
        miss = [n for n, ok in (("내 브섭 링크 (복귀 설정)", self._ret_cfg().get("ps_link")),
                                ("Play 버튼 위치 (접속 설정 → 게임 입장)", pl.get("pos")),
                                ("Click to skip 버튼 위치 (접속 설정 → 게임 입장)", pl.get("skip_pos"))) if not ok]
        return miss

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
                active = self._mod_active() or bool(self.data.get("macro_on"))
                busy = (self.ret.running() or self.play.running() or self.mpop.running()
                        or now < core.EXPECT_CLOSE["until"] or self._mod_busy("crash"))
                if present:
                    seen, gone_since = True, None
                    if self.crash:                     # 다시 켜짐 → 알림 취소
                        self.crash = None
                    continue
                if not active or busy:
                    seen, gone_since = False, None     # 꺼진 상태 · 매크로가 일부러 끄는 중엔 감시 안 함
                    if self.crash and not active:
                        self.crash = None
                    continue
                if self.crash:
                    if now >= self.crash["until"]:
                        self.crash = None
                        seen = False
                        self._on_log("로블록스가 다시 켜지지 않음 — 자동 복구 (매크로 복귀) 실행", "y")
                        self.ret_cause = "crash"
                        self.ret.start("로블록스 꺼짐")
                    continue
                if not seen:
                    continue
                gone_since = gone_since or now
                if now - gone_since >= 1.5:            # 잠깐 사라졌다 다시 뜨는 경우는 무시
                    miss = self._recover_missing()
                    if miss:                           # 복구할 방법이 없으면 알림만 (한 번)
                        seen = False
                        if self.crash_warned != miss:
                            self.crash_warned = miss
                            self._on_log("로블록스가 꺼짐 — 자동 복구 안 함 · 필요한 설정: " + ", ".join(miss), "n")
                        continue
                    self.crash_warned = None
                    self.crash = {"until": now + self.CRASH_WAIT}
                    self._on_log(f"로블록스가 갑자기 꺼짐 — {self.CRASH_WAIT:g}초 뒤 자동 복구 (대기를 누르면 취소)", "y")
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

    # ---------------- 통계 (Acrux 탭) ----------------
    STAT_SAVE_EVERY = 60.0

    def _stat_session(self):
        """이번 실행 기록 (기능마다 켠 뒤부터 센 값)"""
        f, u = dict(self.fisher.stats), dict(self.items.used)
        out = {"fish_success": int(f.get("success", 0)), "fish_junk": int(f.get("junk", 0)),
                "fish_fail": int(f.get("fail", 0)), "fish_full": int(f.get("full", 0)),
                "item_strange": int(u.get("strange", 0)), "item_randomizer": int(u.get("randomizer", 0)),
                "blood_used": int(self.blood.used),
                "merch_bought": int(self.merchant.bought), "crafted": int(self.crafter.crafted),
                "match_played": int(self.matcher.played), "match_ads": int(self.matcher.ads)}
        for extra in self.hooks.emit("stats"):
            if isinstance(extra, dict):
                out.update({str(k): int(v or 0) for k, v in extra.items()})
        return out

    def _stats(self, save=True):
        """이번 실행 · 올타임 — 이번 실행에서 늘어난 만큼 올타임에 더함 (저장은 바뀐 뒤 1분마다)"""
        now = self._stat_session()
        with self.lock:
            st = self.data.get("stats")
            if not isinstance(st, dict):
                st = self.data["stats"] = {}
            changed = False
            for k, v in now.items():
                d = v - self.stat_seen.get(k, 0)
                if d > 0:
                    st[k] = int(st.get(k, 0) or 0) + d
                    changed = True
            self.stat_seen = now
            if changed:
                st.setdefault("since", time.time())
                self.stat_dirty = self.stat_dirty or time.time()
            all_ = {k: int(st.get(k, 0) or 0) for k in now}
            since = st.get("since")
        if save and self.stat_dirty and time.time() - self.stat_dirty >= self.STAT_SAVE_EVERY:
            self.stat_dirty = 0.0
            self._save()
        return {"session": now, "all": all_, "since": since, "started": self.started}

    def save_stats(self):
        """끌 때: 아직 저장 안 한 올타임 기록을 저장"""
        self._stats(save=False)
        if self.stat_dirty:
            self.stat_dirty = 0.0
            self._save()

    def api_poll(self, p):
        self.last_ping = time.time()
        since = int(p.get("since", 0))
        roblox = bool(macro.roblox_window_cached(2.0))     # 창 검색은 잠금 밖에서, 2초 동안 재사용
        stats = self._stats()
        play, bio, ret = self.play.snapshot(), self.biome.state(), self.ret.snapshot()
        mpop, mblood = self.mpop.snapshot(), self.blood.snapshot()
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
            out = {"seq": self.seq, "logs": logs, "status": list(self.status),
                    "play": play, "ret": ret, "mpop": mpop, "mblood": mblood, "mfish": mfish, "roblox": roblox,
                    "move": mv, "mitem": self.items.snapshot(), "mmerch": self.merchant.snapshot(), "mcraft": self.crafter.snapshot(), "mmatch": self.matcher.snapshot(), "macro_on": bool(self.data.get("macro_on")), "online": self.online, "online_on": bool(self.ONLINE_URL),
                    "crash": {"left": max(0.0, self.crash["until"] - time.time())} if self.crash else None,
                    "biome": bio, "stats": stats}
        self.hooks.emit("poll", out, since)
        return out

    def api_set_config(self, p):
        patch = p.get("patch") or {}
        allowed = (set(core.DEFAULT_CONFIG) | self.mod_keys) - {"stats", "mods"}   # 통계는 엔진만 씀
        with self.lock:
            for k, v in patch.items():
                if k in allowed:
                    self.data[k] = v
        self._save()
        return {"config": self.data}

    # ---------------- 게임 입장 (Play 버튼) · 복귀 ----------------
    def _on_ingame(self, path, reason):
        if reason == "복귀":
            self.ret_fails, self.ret_watch = 0, None
            self._after_return("매크로 복귀 완료 — 내 서버 입장")
        else:                               # 모드가 다른 서버로 들어감
            self.hooks.emit("ingame", path, reason)

    def _after_return(self, head):
        """내 서버로 돌아옴: 대기 뒤 원래 매크로 다시 시작 (모드가 대신 할 일이 있으면 모드가 함 · 예: 커스텀 매크로)"""
        r = self.data.get("ret", {})
        wait = float(r.get("start_wait", 7.5))
        self.resume_at = time.time() + wait          # 게임이 다 뜰 때까지 기다렸다가
        self.afk_at = time.time()
        cause, self.ret_cause = self.ret_cause, None
        if cause == "crash":                         # 로블록스가 꺼져서 복구함 → 하던 대로 (커스텀 매크로는 다시 안 돌림)
            on = bool(self.data.get("macro_on"))
            self._on_log("자동 복구 완료 — 내 서버 입장" + (f" · {wait:g}초 뒤 매크로 다시 시작" if on else ""), "g")
            return
        if self.hooks.any("after_return", head, wait):
            return
        self._on_log(f"{head} · {wait:g}초 뒤 매크로 다시 시작", "g")

    RET_RETRY = 2                           # 복귀 Play 가 실패했을 때 다시 복귀하는 횟수

    def _on_play_fail(self, reason):
        if reason != "복귀":               # 모드가 들어가려던 서버에 못 들어감
            self.hooks.emit("play_fail", reason)
        else:              # 내 서버 입장을 확인 못 함 → 다시 복귀 (몇 번까지) · 그래도 안 되면 매크로는 멈춘 채로
            self.ret_fails += 1
            if self.ret_fails <= self.RET_RETRY:
                self._on_log(f"매크로 복귀 실패 — 내 서버 입장이 확인되지 않음 · 다시 복귀 ({self.ret_fails}/{self.RET_RETRY})", "y")
                self.ret.start("복귀 다시 시도")
                return
            self.ret_fails = 0
            self.ret_watch = rejoin.LogWatch()       # 지금부터 게임 입장이 확인되면 매크로 다시 시작
            self._on_log("매크로 복귀 실패 — 매크로는 멈춘 채로 둠 · 내 서버에 직접 들어가거나 F3 으로 매크로를 다시 켜면 다시 시작", "r")

    # ---------------- 매크로 탭 · 자동 낚시 ----------------
    def _macro_loop(self):
        """1초마다: 매크로 버튼 + 자동 낚시 켜기 + 로블록스 창 있음
        + 복귀 · Play 클릭 · 복귀 후 대기 · 다른 모드 작업 없음 → 자동 낚시 돌림, 아니면 멈춤 (돌아오면 다시 켜짐)"""
        warned, paused = None, False
        while True:
            time.sleep(1.0)
            try:
                mf = self.data.get("mfish", {})
                mi = self.data.get("mitem", {})
                want_items = bool(self.data.get("macro_on") and mi.get("enabled"))
                mm = self.data.get("mmerch", {})
                want_merch = bool(self.data.get("macro_on") and mm.get("enabled"))
                want_craft = bool(self.data.get("macro_on") and (self.data.get("mcraft") or {}).get("enabled"))
                want_match = bool(self.data.get("macro_on") and (self.data.get("mmatch") or {}).get("enabled"))
                want_blood = bool(self.data.get("macro_on") and (self.data.get("mblood") or {}).get("enabled"))
                want = bool(self.data.get("macro_on") and mf.get("enabled")) or want_items or want_merch or want_craft or want_match \
                    or want_blood
                rw = self.ret_watch
                if rw is not None and not self.ret.running() and not self.play.running() and rw.in_game():
                    self.ret_watch = None                    # 복귀 실패 뒤 직접 게임에 들어감 → 기다렸다가 매크로 다시 시작
                    self._after_return("게임 입장 확인")
                busy = self.ret.running() or self.play.running() or time.time() < self.resume_at \
                    or self.ret_watch is not None or self._mod_busy("macro")
                if want and busy and not paused:
                    paused = True
                    self._on_log("다른 일 중 (복귀 · 모드) — 매크로 잠시 멈춤 (끝나면 다시 시작)", "c")
                elif paused and (not want or not busy):
                    paused = False
                    if want:
                        self._on_log("내 서버 — 매크로 다시 시작", "g")
                ok = want and not busy and bool(macro.roblox_window_cached(2.0))
                now = time.time()
                if self.merchant_pending and now - self.merchant_pending[1] > 180:
                    self.merchant_pending = None             # 상인이 떠났을 시간
                if self.craft_pending and now - self.craft_pending[1] > 300:
                    self.craft_pending = None
                # 끄거나 다른 작업을 시작하면 하던 기능을 멈춤 (테스트는 끝까지)
                if self.merchant.running() and self.merchant.job == "buy" and (busy or not want_merch):
                    self.merchant.stop()
                if self.crafter.running() and not self.crafter.test and (busy or not want_craft):
                    self.crafter.stop()
                if self.matcher.running() and not self.matcher.test and (busy or not want_match):
                    self.matcher.stop()
                if self.items.running() and not self.items.test and (busy or not want_items):
                    self.items.stop()
                if self.blood.running() and not self.blood.test and (busy or not want_blood):
                    self.blood.stop()
                if ok and want_match and now >= self.matcher.next_at and self._place_of("mmatch", "match_spot") is None:
                    self.matcher.next_at = now + 600         # 장소가 없으면 낚시를 멈추지 않고 알림만 (10분마다)
                    self._on_log("오토 메모리 매치 안 함 — 매크로 설정 → 오토 메모리 매치 → 이동에서 장소 지정 · 시간 재기", "n")
                # ---- 긴급: 블러드 레인 자동 생성 — PUMPKIN MOON · GRAVEYARD 가 떠 있으면 그 바이옴에 한 번 (하던 단일성 기능은 멈추고 먼저)
                if ok and want_blood and not self._emergency():
                    occ = self._blood_due()
                    if occ:
                        self.blood_seen = occ
                        miss = popping.Popper.missing(self._mblood_cfg())
                        if miss:
                            self._on_log(f"블러드 레인 자동 생성 안 함 — 공통 위치 지정 필요: {', '.join(miss)}", "n")
                        else:
                            self._urgent_start(occ[1], "blood")
                            continue
                # ---- 단일성 기능: 자리가 비어 있고 긴급(레어 바이옴 팝핑 · 블러드 레인)이 없으면 신호가 온 순서대로 하나만
                holder, wants = self._single_running(), self._single_wants(ok, want_merch, want_craft, want_match, want_items) if ok else []
                if wants and holder is None and not self._emergency():
                    name, start = wants[0]
                    if name != "items" and self.fisher.running():
                        self.fisher.stop()                   # 다른 곳으로 가야 해서 낚시는 끝냄 (돌아오면 낚시 장소로 다시 감)
                    else:
                        start()
                    continue
                # ---- 가벼운 확인 (자리를 안 씀): 상인 도착 채팅 · Auto Crafted 알림
                if ok and want_merch and holder is None and not self._emergency() and not self.merchant.running() \
                        and now - self.merchant_check_at >= float(mm.get("check_sec", 15)):
                    self.merchant_check_at = now
                    self.merchant.start_job("check")
                if ok and want_craft and not self.crafter.running() and not self.craft_pending and not self.craft_busy \
                        and now - self.craft_check_at >= 2.0:
                    self.craft_check_at, self.craft_busy = now, True    # 글자 읽기(OCR)는 오래 걸릴 수 있어서 따로 (매크로 루프가 안 멈추게)
                    threading.Thread(target=self._craft_notice_bg, daemon=True).start()
                # ---- 상시 기능: 자동 낚시 (단일성 · 긴급 기능이 없을 때만)
                want = bool(self.data.get("macro_on") and mf.get("enabled"))
                ok = ok and want
                if want and fishing.Fisher.missing(mf):
                    ok = False
                    miss = ", ".join(fishing.Fisher.missing(mf))
                    if warned != miss:
                        warned = miss
                        self._on_log(f"자동 낚시 안 함 — 위치 지정 필요 (매크로 설정 → 자동 낚시): {miss}", "n")
                else:
                    warned = None
                if self.fisher.reels != getattr(self, "_reels_seen", 0):   # 낚시가 잘 되면 실패 횟수 · 다시 시도 간격은 처음부터
                    self._reels_seen, self.fish_fails, self.zero_sells, self.fish_backoff = self.fisher.reels, 0, 0, 0
                if self.fish_block and ok and now >= self.fish_retry_at:   # 막힌 뒤 기다렸으면: 리셋 → 낚시 장소로 다시 가서 시도
                    self._on_log(f"자동 낚시 다시 시도 ({self.fish_block}) — 리셋하고 낚시 장소로 다시 이동", "c")
                    self.fish_block, self.fish_fails, self.zero_sells = None, 0, 0
                if self.fish_block:
                    ok = False
                if ok and not self.fisher.running() and self.fisher.failed:     # 낚시 화면을 못 찾고 멈췄음 → 바로 다시 켜기는 한 번만
                    self.fisher.failed = False
                    self.fish_fails += 1
                    if self.fish_fails >= 2:
                        self._block_fish("낚시 화면 없음")
                        ok = False
                if ok and not self.fisher.running() and holder is None and not wants and not self._emergency():
                    self.fisher.start()
                elif not ok and self.fisher.running():
                    self.fisher.stop()
            except Exception as e:
                write_crash(f"macro loop: {e}")

    # 기능 분류
    #  · 긴급: 레어 바이옴 자동 팝핑 · 블러드 레인 자동 생성 — 하던 단일성 기능을 멈추고 먼저 (끝나면 다시)
    #          둘이 겹치면 레어 바이옴 팝핑이 먼저 (블러드 레인을 멈춤)
    #  · 상시: 자동 낚시 — 켜 두면 계속 · 단일성 · 긴급 기능이 자리를 쓰는 동안은 비켜 줌
    #  · 단일성: 상인 자동 구매 · 포션 자동 제작 · 오토 메모리 매치 · 오토 아이템 사용
    #           신호가 오면 한 번 하고 끝 · 자리는 하나라서 한 번에 하나만 (먼저 온 것부터, 끝날 때까지 다른 건 대기)
    def _emergency(self):
        return self.mpop.running() or self.mpop_wait or self.blood.running() or self.blood_wait

    CALS = (("_autocal_running", "_autocal_stop"), ("_sellcal_running", "_sellcal_stop"),
            ("_merchcal_running", "_merchcal_stop"), ("_craftcal_running", "_craftcal_stop"),
            ("_matchcal_running", "_matchcal_stop"))

    def _cal_running(self):
        return any(getattr(self, flag, False) for flag, _ in self.CALS)

    def _single_running(self):
        """지금 자리를 쓰는 단일성 기능 (이동 테스트 · 자동 보정 등 직접 누른 것도 포함) → 이름 또는 None"""
        for name, busy in (("merchant", self.merchant.buying()), ("craft", self.crafter.running()),
                           ("match", self.matcher.running()), ("items", self.items.running()), ("move", self.mover.running()),
                           ("cal", self._cal_running())):
            if busy:
                return name
        return None

    def _fish_yield(self, who, timeout, cancel=True):
        """자동 낚시에게 자리를 달라고 함 → 비켜줬으면(또는 안 돌고 있으면) True
        cancel(급함): 낚시 장소 · 판매 장소로 걷는 중이면 중간에 비켜줄 수 없어서 낚시를 바로 끔
        시간 안에 안 비켜줘도 낚시를 끔 — 끈 낚시는 그 기능이 끝난 뒤 매크로 루프가 다시 켬 (낚시 장소로 다시 감)"""
        f = self.fisher
        if not f.running():
            return True
        walking = bool(self.mover.state.get("ext"))
        if not (cancel and walking):
            if f.hold(timeout, cancel=cancel, who=who):
                return True
            if not f.running():
                return True
            self._on_log(f"자동 낚시가 {timeout:g}초 안에 자리를 비켜주지 않음 — 낚시를 끄고 먼저 진행", "y")
        else:
            self._on_log("자동 낚시가 이동 · 판매 중 — 낚시를 끄고 먼저 진행 (끝나면 다시 낚시)", "y")
        walking = bool(self.mover.state.get("ext"))
        f.stop()
        t = f.thread
        if t is not None:
            t.join(10)
        if f.running():
            return False
        if walking:
            macro.respawn()                 # 걷다가 · 판매 상점 앞에서 멈춤 → 리셋 (열린 상점 창 닫기)
        return True

    def _single_wants(self, ok, want_merch, want_craft, want_match, want_items):
        """자리를 기다리는 단일성 기능 [(이름, 시작 함수)] — 신호가 온 순서(상인 → 포션 → 메모리 매치 → 아이템)"""
        out, now = [], time.time()
        if want_merch and self.merchant_pending:
            def merch(name=self.merchant_pending[0]):
                self.merchant_pending = None
                self.merchant.start_job("buy", name)
            out.append(("merchant", merch))
        if want_craft and self.craft_pending:
            def craft(name=self.craft_pending[0]):
                self.craft_pending = None
                self.crafter.start_job(name)
            out.append(("craft", craft))
        if want_match and now >= self.matcher.next_at and self._place_of("mmatch", "match_spot") is not None:
            out.append(("match", self.matcher.start_job))
        if want_items:
            due = self.items.due(now)
            if due:
                out.append(("items", lambda: self.items.start(due)))
        return out

    def _block_fish(self, why):
        """자동 낚시가 계속 헛돎 → 잠깐 쉬었다가 리셋 · 낚시 장소로 다시 가서 시도 (쉬는 시간은 30초부터 실패할수록 늘림 · 최대 5분)"""
        self.fish_backoff = min(300, max(30, self.fish_backoff * 2))
        self.fish_block, self.fish_retry_at = why, time.time() + self.fish_backoff
        self._on_log(f"자동 낚시 멈춤 ({why}) — {self.fish_backoff}초 뒤 리셋하고 낚시 장소로 다시 가서 시도", "r")

    # 사용자 수: Acrux 가 켜져 있는 동안 몇 분마다 '켜져 있음' 신호 (설치마다 만든 무작위 번호만 보냄)
    # 집계에 참여하지 않으면 신호 없이 숫자만 받아 봄 · 서버 코드는 server/online
    ONLINE_URL = "https://acruxmacro.com/api/"      # 사용자 수 서버 (acruxmacro.com/active 에서 한눈에)

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
                        res = call("/ping", json.dumps({"id": self.data.get("install_id", ""), "app": "pc"}).encode())
                        last_ping = time.time()
                        ping_every = min(3600, max(60, int(res.get("next", 300))))
                    self.online = int(call("/count")["online"])
                except Exception:
                    self.online = None
            time.sleep(60)

    def _set_macro(self, on, why="F3"):
        """매크로 버튼 켜기 · 끄기 (F3 · 화면 버튼과 같음)"""
        self.fish_block, self.fish_fails, self.zero_sells, self.fish_backoff = None, 0, 0, 0
        self.ret_watch = None               # 복귀 실패로 멈춰 둔 것도 풂
        with self.lock:
            self.data["macro_on"] = bool(on)
        self._save()
        if not on:
            self.mpop.stop()
            self.blood.stop()
            self.fisher.stop()
            self.items.stop()
            self.merchant.stop()
            self.crafter.stop()
            self.craft_pending = None
            self.matcher.stop()
        else:
            self.matcher.next_at = 0.0          # 켤 때마다 메모리 매치를 한 번 확인
        self._on_log(f"{why} — 매크로 {'켜짐' if on else '꺼짐'}", "y" if not on else "g")
        self.hooks.emit("macro", bool(on))

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
                want = bool(self.data.get("macro_on")) and (self.fisher.running() or self.mpop.running() or self.blood.running()
                                                          or self.items.running()
                                                          or self.merchant.running() or self.crafter.running()
                                                          or self.matcher.running()) and not other
                alive = banner is not None and banner.poll() is None
                if want and not alive:
                    banner = self._banner_proc("macro")
                elif not want and alive:
                    banner.kill()
                    banner = None
            except Exception as e:
                write_crash(f"hotkey loop: {e}")
                time.sleep(1)

    # ---------------- AFK 방지 (로블록스 20분 튕김) ----------------
    AFK_EVERY = 300                         # 하던 동작이 끝나고 입력이 없으면 5분마다 점프

    def _afk_idle(self):
        """Acrux 가 로블록스에서 아무것도 안 하는 중 (바이옴이 끝나길 기다리는 오토 팝핑은 쉬는 것으로 봄)"""
        busy = (self.play.running() or self.ret.running() or self._mod_busy("afk") or self.mpop.running()
                or self.blood.running() or self.fisher.running() or self.items.running() or self.merchant.running() or self.crafter.running()
                or self.matcher.running() or self.mover.running() or self._cal_running() or self.craft_busy
                or time.time() < self.resume_at)
        return not busy

    AFK_GAP = 0.8                           # 컴퓨터를 쓰는 중이면 입력이 이만큼 멈춘 틈에 점프 (치던 글자가 로블록스로 안 들어가게)
    AFK_GAP_WAIT = 20.0                     # 틈을 이만큼 기다려도 안 나면 그냥 점프

    def _afk_loop(self):
        """동작이 끝나고 5분이 지나면 로블록스에서 점프(Space) 한 번 → 다시 5분 … (Acrux 를 켜 둔 동안만)
        컴퓨터를 쓰는 중이어도 점프 · 다른 창을 보고 있었으면 잠깐 로블록스로 갔다가 그 창으로 바로 돌아감
        로블록스를 직접 하고 있으면(로블록스가 맨 앞 + 입력 있음) 튕길 일이 없어서 안 함"""
        while True:
            time.sleep(2.0)
            try:
                if not (self._mod_active() or self.data.get("macro_on")):
                    continue
                now = time.time()
                if not self._afk_idle():
                    self.afk_at = now                # 동작이 끝난 때부터 다시 셈
                    continue
                if now - self.afk_at < self.AFK_EVERY:
                    continue
                hwnd = macro.roblox_window_cached(2.0)
                if not hwnd:
                    continue
                if macro.is_foreground(hwnd) and macro.idle_seconds() < self.AFK_EVERY:
                    self.afk_at = now                # 로블록스를 직접 하는 중
                    continue
                end = now + self.AFK_GAP_WAIT        # 키보드 · 마우스 입력이 잠깐 멈춘 틈을 기다림
                while macro.idle_seconds() < self.AFK_GAP and time.time() < end:
                    time.sleep(0.05)
                if not self._afk_idle():
                    continue
                back = macro.foreground()
                macro.focus(hwnd, wait=0.15)
                if self._afk_idle():                 # 앞으로 가져오는 사이에 다른 동작이 시작됐으면 안 누름
                    macro.key_tap("space", 80)
                    self._on_log("AFK 방지 — 점프 (5분마다 · 20분 튕김 방지)", "d")
                    time.sleep(0.1)
                    macro.focus_back(back)
                self.afk_at = time.time()
            except Exception as e:
                write_crash(f"afk loop: {e}")
                time.sleep(5)

    def _macro_user_stop(self):
        """F7: 매크로 버튼을 끔 (자동 낚시 · 매크로 탭 기능 전부 멈춤)"""
        with self.lock:
            self.data["macro_on"] = False
        self._save()
        self._on_log("F7 — 매크로 꺼짐", "y")

    def _on_biome_change(self, prev, found):
        """매크로 탭 · 레어 바이옴 자동 팝핑: 지금 켜져 있는 로블록스(내 서버)에서 레어 바이옴이 시작되면 포션 사용
        복귀 · Play 클릭이나 다른 작업이 도는 중이면 기다림"""
        self.hooks.emit("biome", prev, found, False)
        mp = self.data.get("mpop", {})
        if not self.data.get("macro_on") or not mp.get("enabled") or found not in popping.POP_BIOMES:
            return
        if self.ret.running() or self.play.running() or self._mod_busy("mpop"):
            self._on_log(f"{found} 감지 — 다른 기능이 도는 중이라 내 서버 팝핑 안 함", "y")
            return
        if not (mp.get("biomes_on") or {}).get(found, True):
            self._on_log(f"{found} 감지 — 매크로 탭 팝핑 바이옴에서 꺼져 있음", "y")
            return
        if not macro.roblox_window_cached(1.0):
            return                          # 옛 로그 파일 (로블록스가 꺼져 있음)
        if self.mpop.running() or self.mpop_wait:
            self._on_log(f"{found} 감지 — 레어 바이옴 팝핑이 이미 도는 중", "d")
            return
        self._urgent_start(found, "mpop")

    SCREEN_NAMES = {"items": "아이템 사용", "merchant": "상인 구매", "craft": "포션 제작", "match": "메모리 매치",
                    "move": "이동", "cal": "자동 보정", "blood": "블러드 레인"}

    def _screen_users(self, kind="mpop"):
        """지금 화면(마우스 · 키보드)을 쓰는 기능 [(이름, 멈추는 함수)] — 자동 낚시 · 낚시 중 이동 · 판매는 긴급 기능이 직접 비켜 달라고 함
        kind: 누가 묻는지 — 레어 바이옴 팝핑(mpop)이면 돌고 있는 블러드 레인도 멈춤"""
        out = [(name, f.stop) for name, f in (("items", self.items), ("merchant", self.merchant),
                                               ("craft", self.crafter), ("match", self.matcher))
               if (f.buying() if f is self.merchant else f.running())]
        if kind == "mpop" and self.blood.running():
            out.append(("blood", self.blood.stop))
        if self.mover.thread is not None and self.mover.thread.is_alive():
            out.append(("move", self.mover.stop))
        for flag, ev in self.CALS:
            if getattr(self, flag, False) and getattr(self, ev, None) is not None:
                out.append(("cal", getattr(self, ev).set))
        return out

    def _urgent_cancel(self, found, kind):
        """기다리는 사이 시작할 이유가 없어졌는지 (매크로 · 기능을 끔 · 블러드 레인: 레어 바이옴 팝핑이 먼저 · 바이옴이 바뀜)"""
        if not (self.data.get("macro_on") and (self.data.get("mpop" if kind == "mpop" else "mblood") or {}).get("enabled")):
            return True
        return kind == "blood" and (self.mpop.running() or self.mpop_wait or self.biome.current != found)

    def _urgent_start(self, found, kind):
        """긴급 기능 시작 (kind: mpop = 레어 바이옴 자동 팝핑 / blood = 블러드 레인 자동 생성)
        화면을 쓰는 기능(아이템 사용 · 상인 구매 · 제작 · 메모리 매치 · 이동 · 자동 보정)은 멈추고 끝난 뒤 시작 (같이 클릭하면 꼬임)
        기다림 표시를 먼저 걸어서 그 사이 매크로 루프가 다른 기능을 새로 시작하지 않게"""
        runner = self.mpop if kind == "mpop" else self.blood
        setattr(self, kind + "_wait", True)
        with self.urgent_lock:
            if self._urgent_cancel(found, kind):
                setattr(self, kind + "_wait", False)
                return
            if not self._screen_users(kind):
                try:
                    runner.start(found)
                finally:
                    setattr(self, kind + "_wait", False)
                return
        # 멈추는 기능이 자동 낚시가 비켜주길 기다리는 중일 수 있음 → 이 기능 이름으로 급하게 비켜 달라고 미리 걸어 둠 (던진 낚시 취소)
        self.fisher.hold(0, cancel=True, who=kind)
        threading.Thread(target=self._urgent_after_stop, args=(found, kind), daemon=True).start()

    def _urgent_after_stop(self, found, kind):
        """화면을 쓰던 기능을 멈추고, 전부 멈춘 게 확인되면 시작 (100초 안에 안 멈추면 이번엔 취소)
        메모리 매치 광고는 보상을 위해 끝까지 보고 멈추므로 넉넉하게"""
        runner = self.mpop if kind == "mpop" else self.blood
        started = False
        try:
            names, end = [], time.time() + 100
            while True:
                with self.urgent_lock:
                    if self._urgent_cancel(found, kind):
                        return
                    busy = self._screen_users(kind)
                    if not busy:
                        started = runner.start(found)
                        return
                for name, stop in busy:
                    if name not in names:
                        names.append(name)
                        what = self.SCREEN_NAMES.get(name, name)
                        self._on_log(f"{found} 감지 — {what} 멈추고 팝핑 먼저" if kind == "mpop"
                                     else f"{biome.show_name(found)} 감지 — {what} 멈추고 블러드 레인 먼저", "y")
                    stop()
                if time.time() > end:
                    left = ", ".join(self.SCREEN_NAMES.get(n, n) for n, _ in busy)
                    self._on_log(f"{found} 팝핑 취소 — {left} 이(가) 멈추지 않음" if kind == "mpop"
                                 else f"블러드 레인 자동 생성 취소 — {left} 이(가) 멈추지 않음", "r")
                    return
                time.sleep(0.1)
        except Exception as e:
            write_crash(f"{kind} wait: {e}")
        finally:
            if not started:
                self.fisher.release(kind)          # 미리 걸어 둔 '비켜 줘' 풀기 (시작됐으면 끝날 때 풂)
            setattr(self, kind + "_wait", False)

    def _blood_due(self):
        """블러드 레인을 만들 차례인 바이옴 기록 (시각, 바이옴) 또는 None
        지금 내 서버 바이옴이 PUMPKIN MOON · GRAVEYARD 이고, 이번 로그 파일에서 확인했고, 이번에 뜬 것을 아직 처리 안 했으면
        (매크로 · 기능을 그 바이옴 중간에 켜도 한 번 함 · 한 번 뜬 바이옴엔 한 번만)"""
        bw = self.biome
        cur, hist = bw.current, bw.history
        if cur not in popping.BLOOD_BIOMES or not hist or bw.seen_file is None or bw.seen_file != bw.file:
            return None
        occ = hist[-1]
        return occ if occ[1] == cur and occ != self.blood_seen else None

    def api_list_windows(self, _):
        seen, out = set(), []
        for w in macro.list_windows():
            k = (w["title"], w["exe"])
            if k not in seen and w["title"].strip():
                seen.add(k)
                out.append({"title": w["title"], "exe": w["exe"]})
        return {"windows": out}

    def api_steps_info(self, _):
        return {"keys": macro.KEY_NAMES, "types": stepmod.STEP_TYPES}

    def api_combo_check(self, p):
        try:
            return {"keys": stepmod.parse_combo(p.get("keys"))}
        except ValueError as e:
            return {"error": str(e)}

    def api_pick_point_overlay(self, _):
        return self._pick_overlay("--pick-point")

    def api_pick_file(self, _):
        return self._pick_overlay("--pick-file")

    def api_ret_join(self, _):
        """로블록스가 꺼져 있을 때 메인 알림에서: 매크로 복귀 설정의 내 브섭 링크로 바로 접속"""
        link = (self.data.get("ret", {}).get("ps_link") or "").strip()
        if not link:
            return {"error": "복귀 설정에 내 브섭 링크가 없음"}
        try:
            self.ret_watch = None
            core.open_link(link)
        except Exception as e:
            return {"error": f"실행 실패: {e}"}
        return {"ok": True}

    def api_ret_test(self, _):
        if not self._ret_cfg().get("ps_link"):
            return {"error": "복귀할 브섭 링크 먼저 입력"}
        self.hooks.emit("stop")
        self.ret_cause = None
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
        self.ret.stop()
        self.hooks.emit("stop")
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
            return {"error": "공통 위치 지정 필요: " + ", ".join(miss)}
        if not [i for i in ((self.data.get("mpop", {}).get("templates") or {}).get(b) or {}).get("items", [])
                if i.get("name")]:
            return {"error": "포션 목록이 비어 있음"}
        self.mpop.start(b, test=True)
        return {"ok": True}

    # 매크로 기준 위치 설정 — 버튼 위치 · 영역 (feat: base = 여러 기능이 같이 쓰는 기준 위치 / mfish = 자동 낚시만)
    MPOS_POINTS = {"base": dict(popping.POS_KEYS, chat_pos="채팅 버튼", collection_pos="도감 버튼", collection_close="도감 Exit", dialog_pos="대화창"),
                   "mfish": dict(fishing.POS_KEYS, **dict(sell.SELL_KEYS)),
                   "mmerch": dict(merchant.POS_KEYS), "mcraft": dict(crafter.POS_KEYS), "mmatch": dict(memmatch.POS_KEYS)}
    MPOS_REGIONS = {"base": ("ocr_region", "notice_region"), "mfish": ("panel_region", "reel_region", "result_region", "bar_region"),
                    "mmerch": ("chat_region", "item_region"), "mcraft": ("list_region",), "mmatch": ("note_region",)}
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
            if not self.fisher.hold(45, who="cal"):             # 자동 낚시 중이면 안전한 곳(Fish 버튼)에서 잠깐 멈춤
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
            self.fisher.release("cal")
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
        mf["cal_win"] = True                         # 자동 보정을 한 번이라도 끝냄 (튜토리얼 안내 완료)
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
            sold = self.seller.run(stop)
            if sold:
                self.zero_sells = 0
                return True
            # 팔 물고기가 없었음 = 인벤토리가 가득 찬 게 아니라 낚시 자리가 아니었던 것 (Fish 가 반응 없음)
            self.zero_sells += 1
            if self.zero_sells >= 2:
                self._on_log("판매하러 갔는데 두 번 연속 팔 물고기가 없음 — 낚시 자리 문제로 봄", "y")
                self._block_fish("팔 물고기 없음")
                return False
            self._on_log("판매하러 갔는데 팔 물고기가 없음 — 낚시 자리를 다시 잡고 이어감", "y")
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
        대화창(통합 위치) · Sell Fish · 첫 칸 · Sell All · 확인 Sell · 상점 X 를 맞춤 (실제로 팔지는 않음)
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
            if not self.fisher.hold(45, who="cal"):
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
            self.fisher.release("cal")
            self._sellcal_running = False
            macro.focus_back(back)
        if found:                                    # 멈추기 전까지 찾은 위치는 저장
            with self.lock:
                b, mf = self.data.setdefault("base", {}), self.data.setdefault("mfish", {})
                for k, v in found.items():
                    (b if k == "dialog_pos" else mf)[k] = v
            self._save()
        names = dict(self.MPOS_POINTS["base"], **self.MPOS_POINTS["mfish"])
        done = ", ".join(names.get(k, k) for k in found)
        res = {"base": self.data.get("base"), "mfish": self.data.get("mfish"), "found": list(found)}
        if error:
            self._on_log(error + (f" (찾은 것: {done})" if done else ""), "n")
            res["error"] = error
        else:
            with self.lock:
                self.data.setdefault("mfish", {})["cal_sell"] = True
            self._save()
            res["mfish"] = self.data.get("mfish")
            self._on_log(f"판매 자동 보정 완료: {done}" + (" · " + " · ".join(notes) if notes else ""), "g")
            res["done"] = done
            res["notes"] = notes
        return res

    def api_mmerch_autocal(self, _):
        """상인 자동 보정: 상인이 와 있을 때 플레이어가 상인 앞에서 E 를 누르면 대화창 연타 → Open → 상점 글자로
        Open · 상점 위치를 전부 맞춤 → 상점 닫기 (사지는 않음) · F7 이나 버튼을 한 번 더 누르면 취소"""
        if getattr(self, "_merchcal_running", False):
            self._merchcal_stop.set()
            return {"error": "상인 자동 보정 취소 중"}
        if self.mover.running() or self.merchant.running():
            return {"error": "다른 동작이 도는 중"}
        hwnd = macro.roblox_window_cached(1.0)
        if not hwnd:
            return {"error": "로블록스 창 없음"}
        self._merchcal_running, self._merchcal_stop = True, threading.Event()
        stop, back, banner = self._merchcal_stop, macro.foreground(), [None]

        def set_banner(kind):
            if banner[0] is not None and banner[0].poll() is None:
                banner[0].kill()
            banner[0] = self._banner_proc(kind) if kind else None

        def wait(sec):
            end = time.time() + sec
            while True:
                if stop.is_set() or macro.key_down_now("f7"):
                    raise _AutocalStop("상인 자동 보정 취소됨")
                if time.time() >= end:
                    return
                time.sleep(0.03)

        def rect():
            h = macro.roblox_window_cached(1.0)
            r = macro.client_rect(h) if h else None
            if not r:
                raise _AutocalStop("로블록스 창 없음")
            return r

        def click(pos):
            r = rect()
            macro.focus(macro.roblox_window_cached(1.0))
            macro.click(*macro.to_screen(pos[0], pos[1], r))

        def status(msg):
            self._on_log(f"상인 자동 보정 · {msg}", "c")
            if msg.startswith("대화창 찾음"):
                set_banner("autocal")
                self._banner_progress(0.3, 2.0)
            step = {"상점 여는 중": (0.7, 3.0), "상점 닫는 중": (0.95, 1.0)}.get(msg)
            if step:
                self._banner_progress(*step)

        found, error = {}, None
        try:
            if not self.fisher.hold(45, who="cal"):
                return {"error": "자동 낚시가 멈추지 않음 — 낚시를 끄고 다시 눌러주세요"}
            set_banner("merchcal")
            macro.focus(hwnd, wait=0.3)
            found = merchant.autocal(lambda r: macro.ocr_boxes(r), click, wait, rect, status,
                                     (self.data.get("base") or {}).get("dialog_pos"))
        except _AutocalStop as e:
            error = str(e)
        except Exception as e:
            error = f"상인 자동 보정 실패: {e}"
        finally:
            set_banner(None)
            self.fisher.release("cal")
            self._merchcal_running = False
            macro.focus_back(back)
        if found:
            with self.lock:
                self.data.setdefault("mmerch", {}).update(found, **({} if error else {"cal_shop": True}))
            self._save()
        if error:
            self._on_log(error, "n")
            return {"error": error, "mmerch": self.data.get("mmerch")}
        self._on_log("상인 자동 보정 완료", "g")
        return {"mmerch": self.data.get("mmerch")}

    def api_sell_test(self, _):
        """판매 테스트: 기준 장소 → 물고기 판매 장소 → 판매 → 낚시 장소 (자동 낚시는 잠깐 멈췄다가 이어감)"""
        if self.mover.running():
            return {"error": "이동이 이미 도는 중"}
        m = self.seller.missing()
        if m:
            return {"error": m}

        def run():
            try:
                if not self.fisher.hold(45, who="cal"):
                    self._on_log("자동 낚시가 멈추지 않아 판매 테스트를 못 함", "n")
                    return
                self.seller.run()
            except move.Stopped:
                self._on_log("판매 멈춤", "y")
            except Exception as e:
                self._on_log(f"판매 실패: {e}", "n")
            finally:
                self.fisher.release("cal")
        threading.Thread(target=run, daemon=True).start()
        return {"ok": True}

    def api_move_base(self, _):
        miss = self.mover.base_missing()
        if miss:
            return {"error": miss}
        if not self.mover.start("base"):
            return {"error": "이동이 이미 도는 중"}
        return {"ok": True}

    def api_move_base_rev(self, _):
        """반대쪽 기준 장소로 (포션 제작 장소 출발점) — 테스트용"""
        miss = self.mover.base_missing()
        if miss:
            return {"error": miss}
        if not self.mover.start("rev"):
            return {"error": "이동이 이미 도는 중"}
        return {"ok": True}

    def api_move_quest_pick(self, _):
        """퀘스트 보드 Exit 버튼 위치 — 퀘스트 보드 앞에서 E 를 눌러 창을 연 뒤 화면에서 Exit 클릭"""
        r = self._pick_overlay("--pick-point")
        if r.get("error"):
            return r
        with self.lock:
            self.data.setdefault("move", {})["quest_exit_pos"] = [round(r["x"], 4), round(r["y"], 4)]
        self._save()
        return {"move": self.data["move"]}

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

    def api_mblood_test(self, _):
        """블러드 레인 자동 생성 테스트: Cursed Rune Fragment 를 검색해서 개수만 확인 (쓰지 않음)"""
        miss = popping.Popper.missing(self._mblood_cfg())
        if miss:
            return {"error": "공통 위치 지정 필요: " + ", ".join(miss)}
        if self._emergency() or self._single_running():
            return {"error": "다른 동작이 도는 중"}
        self.blood.start(popping.BLOOD_BIOMES[0], test=True)
        return {"ok": True}

    def api_mblood_stop(self, _):
        self.blood.stop()
        return {"ok": True}

    def api_mitem_test(self, _):
        """오토 아이템 사용 테스트: 켜 둔 아이템을 쿨타임과 상관없이 지금 한 번 사용"""
        miss = popping.Popper.missing(self.items._merged())
        if miss:
            return {"error": "공통 위치 지정 필요: " + ", ".join(miss)}
        keys = [k for k, _, _ in popping.ItemUser.ITEMS if (self.data.get("mitem") or {}).get(k)]
        if not keys:
            return {"error": "사용할 아이템이 꺼져 있음"}
        if self.items.running() or self.mover.running():
            return {"error": "다른 동작이 도는 중"}
        self.items.start(keys, test=True)
        return {"ok": True}

    def _place_of(self, feat, key):
        for i, pl in enumerate((self.data.get("move") or {}).get("places") or []):
            if pl.get("feat") == feat and pl.get("key") == key and pl.get("points") \
                    and all(pt.get("pos") and pt.get("time") is not None for pt in pl["points"]):
                return i
        return None

    def _craft_notice_bg(self):
        try:
            self._craft_notice()
        except Exception as e:
            write_crash(f"craft notice: {e}")
        finally:
            self.craft_check_at, self.craft_busy = time.time(), False

    def _craft_notice(self):
        """알림 영역에 하늘색 'Auto Crafted' 알림 → 제작 대기에 올림 (같은 포션은 1분 동안 한 번만)
        제작할 포션을 직접 입력해 뒀으면 그 포션 · 비어 있으면 알림 글자(OCR)에서 포션 이름을 읽음"""
        region = (self.data.get("base") or {}).get("notice_region")
        if not region:
            if not self.craft_warned:
                self.craft_warned = True
                self._on_log("포션 자동 제작 안 함 — 매크로 설정 → 공통 위치 → 알림 영역을 지정해 주세요", "n")
            return
        hwnd = macro.roblox_window_cached(2.0)
        rect = macro.client_rect(hwnd) if hwnd else None
        if not rect:
            return
        with macro.ScreenGrabber() as sct:
            text = macro.notice_check(sct, rect, region, ("crafted", "crofted"), color="blue", min_ratio=0.006)
        if not text:
            return
        typed = crafter.typed_name((self.data.get("mcraft") or {}).get("potion"))
        name = typed or crafter.crafted_name(text)
        if not name:
            if time.time() - getattr(self, "_craft_unread", 0) > 60:
                self._craft_unread = time.time()
                self._on_log("Auto Crafted 알림 — 포션 이름을 못 읽음 · 매크로 설정 → 포션 자동 제작에서 제작할 포션을 입력하면 그 포션으로 함", "y")
            return
        if time.time() - self.craft_seen.get(name, 0) > 60:
            now = time.time()
            self.craft_seen = {k: t for k, t in self.craft_seen.items() if now - t < 600}   # 오래된 기록은 버림
            self.craft_seen[name] = now
            self.craft_pending = (name, time.time())
            self._on_log(f"Auto Crafted 알림 — {name}" + (" (입력한 포션)" if typed else ""), "g")

    def api_mcraft_test(self, p):
        """제작 테스트: 지금 바로 그 포션으로 (이동 → F → 검색 → Open Recipe → Add Everything → Craft → Add Everything)
        이름을 안 보내면 매크로 설정 → 포션 자동 제작에 입력한 '제작할 포션'"""
        name = crafter.typed_name(p.get("name") or (self.data.get("mcraft") or {}).get("potion"))
        if not name:
            return {"error": "제작할 포션을 입력해 주세요"}
        if self._place_of("mcraft", "craft_spot") is None:
            return {"error": "매크로 설정 → 포션 자동 제작 → 이동에서 '포션 제작 장소' 를 먼저 지정 · 시간 재기"}
        if not (self.data.get("move") or {}).get("quest_exit_pos"):
            return {"error": "퀘스트 보드 Exit 위치를 먼저 지정 (매크로 설정 → 포션 자동 제작 → 반대쪽 기준 장소)"}
        if self.crafter.running() or self.mover.running():
            return {"error": "다른 동작이 도는 중"}
        self.fisher.stop()
        self.crafter.start_job(name, test=True)
        return {"ok": True, "name": name}

    def api_mcraft_autocal(self, _):
        """포션 제작 자동 보정: 플레이어가 Stella 앞에서 F 를 누르면 제작 창 → 목록 첫 칸 → Open Recipe → Add Ingredients 를
        글자로 재서 위치를 저장하고 창을 닫음 (재료는 안 넣음) · F7 이나 버튼을 한 번 더 누르면 취소"""
        if getattr(self, "_craftcal_running", False):
            self._craftcal_stop.set()
            return {"error": "포션 제작 자동 보정 취소 중"}
        if self.mover.running() or self.crafter.running():
            return {"error": "다른 동작이 도는 중"}
        hwnd = macro.roblox_window_cached(1.0)
        if not hwnd:
            return {"error": "로블록스 창 없음"}
        self._craftcal_running, self._craftcal_stop = True, threading.Event()
        stop, back, banner = self._craftcal_stop, macro.foreground(), [None]

        def set_banner(kind):
            if banner[0] is not None and banner[0].poll() is None:
                banner[0].kill()
            banner[0] = self._banner_proc(kind) if kind else None

        def wait(sec):
            end = time.time() + sec
            while True:
                if stop.is_set() or macro.key_down_now("f7"):
                    raise _AutocalStop("포션 제작 자동 보정 취소됨")
                if time.time() >= end:
                    return
                time.sleep(0.03)

        def rect():
            h = macro.roblox_window_cached(1.0)
            r = macro.client_rect(h) if h else None
            if not r:
                raise _AutocalStop("로블록스 창 없음")
            return r

        def click(pos):
            r = rect()
            macro.focus(macro.roblox_window_cached(1.0))
            macro.click(*macro.to_screen(pos[0], pos[1], r))

        def status(msg):
            self._on_log(f"포션 제작 자동 보정 · {msg}", "c")
            if msg.startswith("제작 창 찾음"):
                set_banner("autocal")
                self._banner_progress(0.3, 1.5)
            step = {"Open Recipe 확인 중 (재료는 안 넣음)": (0.75, 3.0), "창 닫는 중": (0.95, 1.0)}.get(msg)
            if step:
                self._banner_progress(*step)

        found, error = {}, None
        try:
            if not self.fisher.hold(45, who="cal"):
                return {"error": "자동 낚시가 멈추지 않음 — 낚시를 끄고 다시 눌러주세요"}
            set_banner("craftcal")
            macro.focus(hwnd, wait=0.3)
            found = crafter.autocal(lambda r: macro.ocr_boxes(r), click, wait, rect, status)
        except _AutocalStop as e:
            error = str(e)
        except Exception as e:
            error = f"포션 제작 자동 보정 실패: {e}"
        finally:
            set_banner(None)
            self.fisher.release("cal")
            self._craftcal_running = False
            macro.focus_back(back)
        if found:
            with self.lock:
                self.data.setdefault("mcraft", {}).update(found, **({} if error else {"cal_shop": True}))
            self._save()
        if error:
            self._on_log(error, "n")
            return {"error": error, "mcraft": self.data.get("mcraft")}
        self._on_log("포션 제작 자동 보정 완료", "g")
        return {"mcraft": self.data.get("mcraft")}

    def api_mmatch_autocal(self, _):
        """메모리 매치 자동 보정: 플레이어가 보드 앞에서 E 를 누르면 알림 창 · Start/Available 버튼 · Watch AD 칸 ·
        카드 판 자리를 글자로 재서 저장하고 창을 닫음 (Start · 광고는 안 누름) · F7 이나 버튼을 한 번 더 누르면 취소"""
        if getattr(self, "_matchcal_running", False):
            self._matchcal_stop.set()
            return {"error": "메모리 매치 자동 보정 취소 중"}
        if self.mover.running() or self.matcher.running():
            return {"error": "다른 동작이 도는 중"}
        hwnd = macro.roblox_window_cached(1.0)
        if not hwnd:
            return {"error": "로블록스 창 없음"}
        self._matchcal_running, self._matchcal_stop = True, threading.Event()
        stop, back, banner = self._matchcal_stop, macro.foreground(), [None]

        def set_banner(kind):
            if banner[0] is not None and banner[0].poll() is None:
                banner[0].kill()
            banner[0] = self._banner_proc(kind) if kind else None

        def wait(sec):
            end = time.time() + sec
            while True:
                if stop.is_set() or macro.key_down_now("f7"):
                    raise _AutocalStop("메모리 매치 자동 보정 취소됨")
                if time.time() >= end:
                    return
                time.sleep(0.03)

        def rect():
            h = macro.roblox_window_cached(1.0)
            r = macro.client_rect(h) if h else None
            if not r:
                raise _AutocalStop("로블록스 창 없음")
            return r

        def click(pos):
            r = rect()
            macro.focus(macro.roblox_window_cached(1.0))
            macro.click(*macro.to_screen(pos[0], pos[1], r))

        def status(msg):
            self._on_log(f"메모리 매치 자동 보정 · {msg}", "c")
            if msg.startswith("메모리 매치 창 찾음"):
                set_banner("autocal")
                self._banner_progress(0.6, 1.5)
            elif msg == "창 닫는 중":
                self._banner_progress(0.95, 1.0)

        found, notes, error = {}, [], None
        try:
            if not self.fisher.hold(45, who="cal"):
                return {"error": "자동 낚시가 멈추지 않음 — 낚시를 끄고 다시 눌러주세요"}
            set_banner("matchcal")
            macro.focus(hwnd, wait=0.3)
            found, notes = memmatch.autocal(lambda r: macro.ocr_boxes(r), click, wait, rect, status)
        except _AutocalStop as e:
            error = str(e)
        except Exception as e:
            error = f"메모리 매치 자동 보정 실패: {e}"
        finally:
            set_banner(None)
            self.fisher.release("cal")
            self._matchcal_running = False
            macro.focus_back(back)
        if found:
            with self.lock:
                self.data.setdefault("mmatch", {}).update(found, **({} if error else {"cal_note": True}))
            self._save()
        if error:
            self._on_log(error, "n")
            return {"error": error, "mmatch": self.data.get("mmatch")}
        self._on_log("메모리 매치 자동 보정 완료" + (" · " + " · ".join(notes) if notes else ""), "g")
        return {"mmatch": self.data.get("mmatch"), "notes": notes}

    def api_mmatch_boardcal(self, _):
        """카드 판 자동 보정: 메모리 매치 카드 판이 떠 있을 때 누르면 카드 20장 · 남은 기회 · Close 자리를 글자로 재서 저장"""
        if self.matcher.running():
            return {"error": "오토 메모리 매치가 도는 중 — 멈춘 뒤 눌러주세요"}
        hwnd = macro.roblox_window_cached(1.0)
        rect = macro.client_rect(hwnd) if hwnd else None
        if not rect:
            return {"error": "로블록스 창 없음"}
        try:
            macro.focus(hwnd, wait=0.3)
            found = memmatch.board_autocal(lambda r: macro.ocr_boxes(r), macro.client_rect(hwnd))
        except Exception as e:
            self._on_log(f"카드 판 자동 보정 실패: {e}", "n")
            return {"error": f"카드 판 자동 보정 실패: {e}"}
        with self.lock:
            self.data.setdefault("mmatch", {}).update(found)
        self._save()
        self._on_log("카드 판 자동 보정 완료", "g")
        return {"mmatch": self.data.get("mmatch")}

    def api_mmatch_test(self, _):
        """메모리 매치 테스트: 지금 자리에서 (메모리 매치 보드 앞) E → 확인 → 할 수 있으면 짝 맞추기"""
        if self.matcher.running() or self.mover.running():
            return {"error": "다른 동작이 도는 중"}
        self.fisher.stop()
        self.matcher.start_job(here=True, test=True)
        return {"ok": True}

    def api_mmatch_stop(self, _):
        self.matcher.stop()
        return {"ok": True}

    def api_mcraft_stop(self, _):
        self.crafter.stop()
        self.craft_pending = None
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
            return {"error": "채팅 글자 영역 지정 필요 (매크로 설정 → 상인 자동 구매 → 채팅)"}
        if self.merchant.running():
            return {"error": "상인 자동 구매가 도는 중"}
        self.merchant.seen.clear()
        self.merchant.counts = {}            # 테스트: 채팅에 보이는 도착 줄을 전부 새로 온 것으로
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
        limit = 300 if flag in ("--pick-file", "--pick-mod") else 150
        self.fisher.no_focus = True             # 선택 창이 떠 있는 동안 자동 낚시가 로블록스를 앞으로 끌어오지 않게
        try:
            proc = subprocess.Popen(cmd, creationflags=NO_WINDOW, env=env)
            self._pick_proc = proc
            start = time.time()
            while proc.poll() is None:
                waited = time.time() - start
                # 프로그램 고르기 창은 윈도우 기본 창이라 신호를 안 보냄 → 시간만 봄
                if flag not in ("--pick-file", "--pick-mod") and waited > 6 and not shown.exists():
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

    # 로블록스 꺼짐 감시 (앱이 켜지면 바로)
    def start_watch(self):
        if not getattr(self, "_crash_thread", None):
            self._crash_thread = threading.Thread(target=self._crash_watch, daemon=True)
            self._crash_thread.start()

    # ---------------- 모드 · 플러그인 (mods.py) ----------------
    def api_mods_list(self, _):
        return {"mods": modmod.listing(self), "folder": str(modmod.MOD_DIR)}

    def api_mods_toggle(self, p):
        mid = str(p.get("id") or "")
        if mid not in {m.id for m in self.mods}:
            return {"error": "설치되지 않은 모드"}
        with self.lock:
            self.data.setdefault("mods", {})[mid] = {"enabled": bool(p.get("on"))}
        self._save()
        return {"mods": modmod.listing(self), "restart": True}

    def api_mods_install(self, p):
        """모드 파일 고르기 → 설치 (path 를 주면 그 파일) · 다시 켜면 적용"""
        path = str(p.get("path") or "")
        if not path:
            r = self._pick_overlay("--pick-mod")
            if r.get("error") == "취소됨":
                return {"cancel": True}
            if r.get("error"):
                return r
            path = r.get("path") or ""
            if not path:
                return {"cancel": True}
        try:
            info = modmod.install(path)
        except Exception as e:
            return {"error": f"설치 못 함: {e}"}
        with self.lock:
            self.data.setdefault("mods", {})[info["id"]] = {"enabled": True}
        self._save()
        name = info.get("name") or info["id"]
        self._on_log(f"모드 설치: {name} V{info.get('version') or '?'} — Acrux 를 다시 켜면 적용", "g")
        self.mods = [m for m in self.mods if m.id != info["id"]] + [m for m in modmod.scan() if m.id == info["id"]]
        return {"ok": True, "name": name, "restart": True, "mods": modmod.listing(self)}

    def api_mods_remove(self, p):
        mid = str(p.get("id") or "")
        try:
            modmod.remove(mid)
        except Exception as e:
            return {"error": str(e)}
        with self.lock:
            (self.data.get("mods") or {}).pop(mid, None)
        self._save()
        self.mods = [m for m in self.mods if m.id != mid]
        self._on_log(f"모드 삭제: {mid} — Acrux 를 다시 켜면 적용", "y")
        return {"ok": True, "restart": True, "mods": modmod.listing(self)}

    def api_mods_folder(self, _):
        modmod.MOD_DIR.mkdir(parents=True, exist_ok=True)
        os.startfile(str(modmod.MOD_DIR))
        return {"ok": True}

    def api_restart(self, _):
        """Acrux 다시 켜기 (모드 켜기 · 끄기 · 설치 적용) — 새로 띄운 뒤 지금 것은 끔"""
        exe = Path(sys.executable)
        cmd = [str(exe)] if exe.name.lower().startswith("acruxhost") else [str(exe), str(Path(__file__).resolve())]
        try:
            subprocess.Popen(cmd, cwd=str(Path(__file__).resolve().parent), creationflags=NO_WINDOW, close_fds=True)
        except Exception as e:
            return {"error": f"다시 켜지 못함: {e}"}

        def bye():
            time.sleep(0.4)
            shutdown()
            p = getattr(self, "edge_proc", None)
            if p and p.poll() is None:
                subprocess.run(["taskkill", "/PID", str(p.pid), "/T", "/F"], capture_output=True, creationflags=NO_WINDOW)
            os._exit(0)
        threading.Thread(target=bye, daemon=True).start()
        return {"ok": True}

    # 크레딧 링크 (정해진 주소만 열기)
    WEB_LINKS = {
        "discord": "https://discord.gg/cyK26gsjCC",
        "roblox": "https://www.roblox.com/ko/users/11743130211/profile",
        "donate": "https://ko-fi.com/acrux_",
        "web": "https://acruxmacro.com",
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
            shutdown()
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
            if not notes["ko"] and not notes["en"]:
                continue                                    # 설명이 없는 릴리스는 안 보여줌
            entries.append({"version": x["tag_name"][1:], "date": str(x.get("published_at") or "")[:10], **notes})
        entries.sort(key=lambda e: [int(n) if n.isdigit() else 0 for n in e["version"].split(".")], reverse=True)
        out = {"entries": entries, "current": VERSION}
        Bridge._changelog_cache = (time.time(), out)
        return out

    def api_ocr_info(self, _):
        """Acrux 설정 · OCR 감지 방식: 지금 쓰는 엔진 · RapidOCR 설치 여부"""
        rapid = macro.rapid_available()
        return {"engine": macro.ocr_engine_name(), "rapid": rapid, "mode": macro.OCR_MODE["mode"],
                "rapid_error": None if rapid else macro._OCR.failed}

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
        if path.startswith("/mods/"):            # 모드 화면 파일 (mods.py)
            got = modmod.serve(path)
            return self._send(200, got[0], got[1]) if got else self._send(404, {"error": "not found"})
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

def _clean_install_leftovers():
    """런처로 설치한 경우: 업데이트 중 파일이 잠겨서 못 지운 예전 폴더(runtime.old 등) · 받다 만 zip 정리"""
    if not os.environ.get("ACRUX_DATA"):
        return
    import shutil
    root = Path(os.environ["ACRUX_DATA"]).parent
    for name in ("runtime.old", "runtime.new", "app.old", "app.new"):
        shutil.rmtree(root / name, ignore_errors=True)
    for z in root.glob("tmp*.zip"):
        try:
            if time.time() - z.stat().st_mtime > 3600:
                z.unlink()
        except OSError:
            pass

def main():
    global BRIDGE
    BRIDGE = Bridge()
    modmod.load_all(BRIDGE)               # 모드 · 플러그인 (데이터 폴더 → mods)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    port = server.server_address[1]
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{port}/#{TOKEN}"

    threading.Thread(target=_clean_install_leftovers, daemon=True).start()
    BRIDGE.start_watch()                  # 로블록스 꺼짐 감시
    # OCR 엔진은 미리 불러두지 않음 — 글자를 읽어야 할 때만 따로 띄우고, 안 쓰면 꺼짐 (메모리 절약)
    BRIDGE._on_log(f"OCR 엔진: {macro.ocr_engine_name()}", "d")
    BRIDGE.hooks.emit("ready")

    edge = find_edge()
    proc = None
    if edge:
        DATA_DIR.mkdir(parents=True, exist_ok=True)
        # 화면은 이 PC 안의 페이지 하나뿐 → 업데이트 확인 · 동기화 · 미리 띄워 두는 여분 프로세스 등 쓸데없는 일은 끔 (메모리 절약)
        proc = subprocess.Popen([str(edge), f"--app={url}", f"--user-data-dir={DATA_DIR / 'ui'}",
                                 f"--window-size={WIN_W},{WIN_H}", "--no-first-run", "--no-default-browser-check",
                                 "--disable-features=Translate,msEdgeSidebarV2,SpareRendererForSitePerProcess,"
                                 "msEdgeShoppingUI,msShoppingExp,msEdgeCollections,msUndersideButton,msWebAssist,"
                                 "msEdgeEnhanceImages,EdgeWorkspaces,OptimizationHints,OptimizationGuideModelDownloading,"
                                 "MediaRouter,DialMediaRouteProvider,AutofillServerCommunication,BackForwardCache",
                                 "--renderer-process-limit=1", "--disk-cache-size=1048576", "--disable-gpu-shader-disk-cache",
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

_SHUT = threading.Lock()

def shutdown(server=None):
    """프로그램을 끌 때: 모드에 알림 → 통계 저장"""
    if not _SHUT.acquire(blocking=False):
        return
    core.STOP.set()
    try:
        if BRIDGE:
            BRIDGE.hooks.emit("shutdown")
    except Exception:
        pass
    try:
        macro._OCR.close()                    # OCR 일꾼도 끔
    except Exception:
        pass
    try:
        if BRIDGE:
            BRIDGE.save_stats()               # 통계 (올타임) 마지막 몫
    except Exception:
        pass
    if server:
        server.shutdown()

if __name__ == "__main__":
    if "--pick-region" in sys.argv:          # exe 로 묶었을 때 영역 지정 창으로 다시 실행되는 경우
        macro.pick_region_to_file(sys.argv[sys.argv.index("--pick-region") + 1])
        sys.exit(0)
    if "--pick-file" in sys.argv:            # 프로그램 고르기 창
        macro.pick_file_to_file(sys.argv[sys.argv.index("--pick-file") + 1])
        sys.exit(0)
    if "--pick-mod" in sys.argv:             # 모드 파일 고르기 창
        macro.pick_file_to_file(sys.argv[sys.argv.index("--pick-mod") + 1], "mod")
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
