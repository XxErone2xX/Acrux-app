# -*- coding: utf-8 -*-
"""
오토 팝핑 — Play 버튼 자동 클릭
1. 로블록스 창이 뜰 때까지 기다림
2. 창을 맨 앞으로 → 지정한 Play 버튼 위치를 일정 간격으로 클릭
3. 로블록스 로그에 '"state":"Equipped' 가 새로 찍히면(= 게임에 들어감) 멈춤
"""
import threading
import time

import macro
import biome

INGAME_MARK = '"state":"Equipped'


class LogWatch:
    """시작 시점 이후에 새로 찍힌 로그 줄만 확인"""

    def __init__(self, log_dir=None, new_only=False):
        self.log_dir = log_dir
        self.new_only = new_only            # 서버 접속 시: 새로 실행된 로블록스의 로그만 인정 (이전 클라이언트 기록 무시)
        self.pos = {}
        self.hit_file = None
        p = biome.latest_log(log_dir)
        self.base = p
        if p:
            try:
                self.pos[p] = p.stat().st_size      # 기존 파일은 지금 크기부터
            except OSError:
                pass

    def in_game(self):
        p = biome.latest_log(self.log_dir)
        if not p:
            return False
        start = self.pos.get(p, 0)                  # 새로 생긴 파일은 처음부터
        try:
            with open(p, "r", encoding="utf-8", errors="ignore") as f:
                f.seek(start)
                text = f.read()
                self.pos[p] = f.tell()
        except OSError:
            return False
        if self.new_only and p == self.base:
            return False
        if INGAME_MARK in text:
            self.hit_file = p                       # 게임에 들어간 로그 파일 (오토 팝핑의 바이옴 확인용)
            return True
        return False


class PlayClicker:
    def __init__(self, get_cfg, log, log_dir=None, on_ingame=None, on_fail=None):
        self.get_cfg = get_cfg          # () -> play 설정 dict
        self.on_ingame = on_ingame      # (로그 파일, 이유) — 게임 입장 확인 후 (오토 팝핑 시작)
        self.on_fail = on_fail          # (이유) — 입장 못 하고 끝남 (사용자가 멈춘 경우 제외)
        self.log = log                  # (msg, color)
        self.log_dir = log_dir
        self.stop_ev = threading.Event()
        self.thread = None
        self.lock = threading.Lock()
        self.state = {"running": False, "msg": "대기", "clicks": 0}
        self.reason = ""                # 지금(마지막) 클릭 이유: 서버 접속 · 복귀 · 테스트
        self.gen = 0                    # 시작할 때마다 +1 — 그 사이 새로 시작했으면 옛 클릭의 실패 알림은 버림

    def running(self):
        return bool(self.thread and self.thread.is_alive())

    def snapshot(self):
        with self.lock:
            return dict(self.state, running=self.running())

    def _set(self, **kw):
        with self.lock:
            self.state.update(kw)

    def stop(self):
        self.stop_ev.set()

    def start(self, reason="", load_wait=None):
        """이미 돌고 있으면 멈추고 새로 시작"""
        if self.running():
            self.stop()
            self.thread.join(3)
        self.stop_ev = threading.Event()
        self.reason = reason
        self.gen += 1
        watch = LogWatch(self.log_dir, new_only=reason != "테스트")   # 실행 직전 로그 위치 기억
        self.thread = threading.Thread(target=self._run, args=(reason, load_wait, watch, self.stop_ev, self.gen),
                                       daemon=True)
        self.thread.start()
        return True

    def _sleep(self, sec, stop):
        return stop.wait(max(0.0, sec))

    def _run(self, reason, load_wait, watch, stop, gen=0):
        cfg = self.get_cfg() or {}
        pos, skip = cfg.get("pos"), cfg.get("skip_pos")
        if not pos or not skip:
            miss = " · ".join(n for n, v in (("Play 버튼", pos), ("Click to skip 버튼", skip)) if not v)
            self.log(f"{miss} 위치가 지정되지 않아 클릭 안 함 (오토 팝핑 매크로 설정에서 지정)", "n")
            return
        targets = [("Play", pos), ("Click to skip", skip)]   # 번갈아 클릭
        interval = max(0.05, float(cfg.get("interval", 0.15)))
        max_time = max(10.0, float(cfg.get("max_time", 300)))
        wait0 = float(cfg.get("load_wait", 5) if load_wait is None else load_wait)
        self._set(clicks=0, msg="로블록스 창 기다리는 중")
        self.log(f"Play 버튼 자동 클릭 시작{' (' + reason + ')' if reason else ''}", "c")
        try:
            hwnd = macro.wait_for_roblox(90, stop)
            if not hwnd:
                if not stop.is_set():
                    self.log("로블록스 창이 안 떠서 Play 클릭 취소", "r")
                return
            # 스나이핑으로 클라이언트가 새로 뜬 경우: 바로 맨 앞으로 끌어옴
            macro.focus(hwnd, wait=0.1)
            fg = "맨 앞으로 가져옴" if macro.is_foreground(hwnd) else "맨 앞으로 못 가져옴"
            self.log(f"로블록스 창 감지 ({fg}) — {wait0:g}초 뒤 Play / Click to skip 클릭", "d")
            if wait0 > 0:
                self._set(msg=f"로딩 대기 {wait0:g}초")
                if self._sleep(wait0, stop):
                    return
            end = time.time() + max_time
            clicks = 0
            # 먼저 클릭하고 → 그 뒤에 로그 확인 (클릭 전에 찍힌 기록은 무시)
            watch.in_game()
            with macro.fast_timing():                # 클릭하는 동안만 타이머 1ms + 우선순위 높음
                t_next = time.time()                 # 클릭 시간표 (렉으로 밀려도 다음 클릭에서 따라잡음)
                while not stop.is_set() and time.time() < end:
                    hwnd = macro.roblox_window_cached()
                    rect = macro.client_rect(hwnd) if hwnd else None
                    if not rect or rect[2] < 50 or rect[3] < 50:
                        self._set(msg="로블록스 창 없음")
                        if self._sleep(1, stop):
                            return
                        t_next = time.time()
                        continue
                    name, tp = targets[clicks % 2]
                    macro.focus(hwnd, wait=0.3 if clicks == 0 else 0)
                    if clicks == 0:
                        t_next = time.time()
                    x, y = macro.to_screen(tp[0], tp[1], rect)
                    macro.move_to(x + 3, y + 3)      # 로블록스 버튼이 마우스를 인식하도록 근처로 옮겼다가
                    time.sleep(0.03)
                    macro.click(x, y, hold_ms=30)
                    clicks += 1
                    if clicks <= 2:
                        fg = "앞에 있음" if macro.is_foreground(hwnd) else "앞으로 못 가져옴"
                        self.log(f"{name} 클릭 — 화면 좌표 ({x}, {y}) · 로블록스 창 {rect[2]}x{rect[3]} · {fg}", "d")
                    self._set(clicks=clicks, msg=f"{name} 클릭 ({clicks}번째) · 입장 확인 중")
                    # 다음 클릭 시각: 시간표 기준. 렉으로 한 간격 넘게 밀렸으면 몰아서 누르지 않고 지금부터 다시
                    t_next += interval
                    if time.time() - t_next > interval:
                        t_next = time.time()
                    first = True
                    while first or time.time() < t_next:
                        first = False
                        if watch.in_game():
                            self.log(f"게임 입장 확인 — Play 클릭 종료 (클릭 {clicks}번)", "g")
                            if self.on_ingame and reason != "테스트":
                                threading.Thread(target=self.on_ingame, args=(watch.hit_file, reason),
                                                 daemon=True).start()
                            return
                        left = t_next - time.time()
                        if left > 0 and self._sleep(min(0.25, left), stop):
                            return
                        if stop.is_set():
                            return
            if not stop.is_set():
                self.log(f"{int(max_time)}초 동안 게임 입장이 확인되지 않아 Play 클릭 중지", "y")
        except Exception as e:
            self.log(f"Play 클릭 오류: {e}", "r")
        finally:
            if stop.is_set():
                self.log("Play 클릭 정지", "d")
            self._set(msg="대기")
            if not watch.hit_file and not stop.is_set() and self.on_fail and reason != "테스트" and gen == self.gen:
                def fail():
                    if gen == self.gen:             # 그 사이 새 접속(스나이핑)이 시작됐으면 무시
                        self.on_fail(reason)
                threading.Thread(target=fail, daemon=True).start()


class Returner:
    """매크로 복귀: 모든 로블록스 클라이언트 종료 → 1초 → 내 브섭 링크로 접속 → Play 클릭"""

    def __init__(self, get_cfg, log, play, kill, launch, before_launch=None):
        self.get_cfg = get_cfg          # () -> ret 설정 dict
        self.log = log
        self.play = play                # PlayClicker
        self.kill = kill                # () -> 로블록스 전부 종료
        self.launch = launch            # (링크) -> 실행
        self.before_launch = before_launch   # () — 내 서버 링크를 열기 직전 (바이옴 웹후크 다시 켜기)
        self.stop_ev = threading.Event()
        self.thread = None
        self.lock = threading.Lock()
        self.state = {"running": False, "msg": "대기"}

    def running(self):
        return bool(self.thread and self.thread.is_alive())

    def snapshot(self):
        with self.lock:
            return dict(self.state, running=self.running())

    def _set(self, **kw):
        with self.lock:
            self.state.update(kw)

    def stop(self):
        self.stop_ev.set()

    def start(self, reason=""):
        if self.running():
            return False
        self.stop_ev = threading.Event()
        self.thread = threading.Thread(target=self._run, args=(reason, self.stop_ev), daemon=True)
        self.thread.start()
        return True

    def _run(self, reason, stop):
        link = ((self.get_cfg() or {}).get("ps_link") or "").strip()
        if not link:
            self.log("매크로 복귀 안 함 — 복귀할 브섭 링크가 없음 (매크로 복귀 설정에서 입력)", "n")
            return
        try:
            self.log(f"매크로 복귀 시작{' (' + reason + ')' if reason else ''} — 로블록스 종료", "c")
            self._set(msg="로블록스 종료")
            self.kill()
            self._set(msg="1초 대기")
            if stop.wait(1.0):
                self.log("매크로 복귀 정지", "d")
                return
            self._set(msg="내 서버로 접속")
            if self.before_launch:
                self.before_launch()
            self.launch(link)
            self.play.start("복귀")
        except Exception as e:
            self.log(f"매크로 복귀 오류: {e}", "r")
        finally:
            self._set(msg="대기")
