# -*- coding: utf-8 -*-
"""
이동 — 기준 장소에서 화면의 한 점을 눌러(Click to Move) 원하는 장소로 걸어감
외부 앱이라 게임 안 좌표를 모르므로, 매번 '같은 화면'을 만든 뒤 그 화면의 같은 점을 누름:
  1. 기준 장소: 리셋(Esc → R → Enter) → / · 채팅 · 도감 열고 닫기 · / · Enter (카메라 정렬)
     → W 0.85초 → W+A 8초 (구석으로 걸어가 항상 같은 자리) → 우클릭 드래그(위에서 내려다보기) → O 2.5초 (최대 줌)
  2. 장소마다 지점 목록: [누를 곳, 걸리는 시간] — 지점이 여러 개면 앞 지점에 도착한 화면에서 다음 지점을 누름
  3. 걸리는 시간은 직접 잼: 누른 순간부터 사용자가 '도착'(F6 또는 화면의 버튼)을 누를 때까지
"""
import threading
import time

import macro

ARRIVE_KEY = "f6"        # 시간 잴 때 도착하면 누르는 키
MEASURE_MAX = 180.0      # 시간 재기 최대 (초)
MOVE_KEYS = ("w", "a", "s", "d", "o", "space", "shift", "e")


class Stopped(Exception):
    pass


class Mover:
    LABEL = "이동"

    def __init__(self, get_base, get_move, log, set_move_time=None, before=None, after=None, banner=None):
        self.get_base = get_base            # 통합 위치 (Collection 버튼 · 닫기 위치)
        self.get_move = get_move            # 이동 설정 (move)
        self.log = log
        self.set_move_time = set_move_time  # (장소, 지점, 초) → 잰 시간 저장
        self.before, self.after = before, after   # 자동 낚시 잠깐 멈춤 / 이어감
        self.banner = banner                # 화면 위 안내 띠: banner('move' · 'measure' · None)
        self.stop_ev = threading.Event()
        self.arrive_ev = threading.Event()
        self.thread = None
        self.lock = threading.Lock()
        self.state = {"msg": "대기", "measuring": None, "error": None}

    # ---- 상태
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

    def arrive(self):
        """시간 재는 중: 도착 (화면의 [도착] 버튼)"""
        self.arrive_ev.set()

    # ---- 실행 (별도 스레드)
    def start(self, job, *args):
        """job: 'base' (기준 장소로만) / 'test' (장소 i, 지점 j, 기준 장소부터?) / 'place' (장소 i 로 이동)"""
        if self.running():
            return False
        self.stop_ev = threading.Event()
        self.arrive_ev.clear()
        self._set(error=None)
        self.thread = threading.Thread(target=self._run, args=(job, args), daemon=True)
        self.thread.start()
        return True

    def _run(self, job, args):
        held = False
        try:
            if self.before:
                held = True
                self.before()
            self._banner("move")
            if job == "base":
                self.go_base()
                self.log("기준 장소 도착", "g")
            elif job == "place":
                self.go_place(*args)
            elif job == "test":
                self.measure(*args)
            elif job == "prep":                     # 지점 지정 준비: 기준 장소 → 앞 지점들까지
                i, j = args
                self.go_base()
                self.walk(i, upto=j)
        except Stopped:
            self.log("이동 멈춤", "y")
        except Exception as e:
            self._set(error=str(e))
            self.log(f"이동 오류: {e}", "n")
        finally:
            self._release_keys()
            self._banner(None)
            self._set(msg="대기", measuring=None)
            if held and self.after:
                self.after()

    # ---- 도우미
    def _banner(self, kind):
        if self.banner:
            try:
                self.banner(kind)
            except Exception:
                pass

    def _check(self):
        if self.stop_ev.is_set() or macro.key_down_now("f7"):
            self.stop_ev.set()
            raise Stopped()

    def _wait(self, sec):
        end = time.time() + max(0.0, float(sec))
        while True:
            self._check()
            left = end - time.time()
            if left <= 0:
                return
            time.sleep(min(0.03, left))

    def _rect(self):
        hwnd = macro.roblox_window_cached(1.0)
        rect = macro.client_rect(hwnd) if hwnd else None
        if not rect or rect[2] < 50:
            raise RuntimeError("로블록스 창을 찾을 수 없음")
        macro.focus(hwnd)
        return rect

    def _click(self, pos, button="left"):
        rect = self._rect()
        x, y = macro.to_screen(pos[0], pos[1], rect)
        macro.click(x, y, button=button)

    @staticmethod
    def _release_keys():
        for k in MOVE_KEYS:
            try:
                macro.key_up(k)
            except Exception:
                pass
        try:
            macro.mouse_button("right", False)      # 화면 돌리던 우클릭이 눌린 채 남지 않게
        except Exception:
            pass

    # ---- 미리 확인 (시작하기 전에 알림으로 알려 주려고)
    BASE_KEYS = (("chat_pos", "채팅 버튼"), ("collection_pos", "도감 버튼"), ("collection_close", "도감 Exit"))

    def base_missing(self):
        b = self.get_base() or {}
        miss = [n for k, n in self.BASE_KEYS if not b.get(k)]
        return f"먼저 지정 필요: {', '.join(miss)} (매크로 기준 위치 설정)" if miss else None

    def path_missing(self, i, upto=None):
        """장소 i 의 지점들(upto 앞까지) 중 위치 · 시간이 없는 것 → 안내 글 또는 None"""
        places = (self.get_move() or {}).get("places") or []
        if not 0 <= i < len(places):
            return "장소를 찾을 수 없음"
        pl = places[i]
        for n, pt in enumerate(pl["points"][:upto] if upto is not None else pl["points"], 1):
            if not pt.get("pos"):
                return f"{pl['name'] or '장소'} · {n}번 지점 위치가 없음"
            if pt.get("time") is None:
                return f"{pl['name'] or '장소'} · {n}번 지점 시간을 먼저 재야 함 (테스트)"
        return None

    # ---- 1. 기준 장소 (사용자가 정한 순서)
    def _hold(self, keys, sec):
        """키들을 sec 초 동안 누르고 있다가 뗌 (멈추면 바로 뗌)
        진짜 키보드처럼 누르는 동안 '누름' 신호를 계속 다시 보냄 (약 30번/초) — 한 번만 보내면
        누를 때마다 한 칸씩 움직이는 것(O 키 줌 · 화면 기울이기 등)은 한 칸만 움직이고 끝났음"""
        try:
            for k in keys:
                macro.key_down(k)
            end = time.time() + max(0.0, float(sec))
            while time.time() < end:
                self._check()
                time.sleep(min(0.033, max(0.0, end - time.time())))
                if time.time() < end:
                    for k in keys:
                        macro.key_down(k)          # 자동 반복 (키보드를 꾹 누를 때와 같음)
        finally:
            for k in reversed(keys):
                macro.key_up(k)

    def _tilt_down(self, px):
        """화면을 위에서 아래로 내려다보게: 화면 가운데에서 우클릭을 누른 채 마우스를 아래로 끌기 (로블록스 카메라 돌리기)
        한 번에 크게 옮기면 게임이 놓칠 수 있어서 20px 씩 나눠서 · 카메라는 끝까지 가면 멈추므로 넉넉히 끌어도 됨"""
        if px <= 0:
            return
        rect = self._rect()
        macro.move_to(rect[0] + rect[2] // 2, rect[1] + rect[3] // 2)
        self._wait(0.1)
        try:
            macro.mouse_button("right", True)
            self._wait(0.05)
            done = 0
            while done < px:
                self._check()
                step = min(20, px - done)
                macro.move_rel(0, step)
                done += step
                time.sleep(0.01)
            self._wait(0.05)
        finally:
            macro.mouse_button("right", False)

    def go_base(self):
        """Esc → R → Enter (리셋 · 0.5초 간격) → 3.5초 → / → 채팅 버튼 → 도감 버튼 → 도감 Exit → / → Enter (각 0.5초)
        → W 0.85초 → W+A 8초 → 0.5초 → 우클릭 드래그로 위에서 내려다보기 → O 2.5초 (최대 줌)"""
        b, mv = self.get_base() or {}, self.get_move() or {}
        miss = self.base_missing()
        if miss:
            raise RuntimeError(miss)
        self._set(msg="기준 장소로 이동 · 리셋")
        self._release_keys()
        self._rect()
        self._wait(0.2)
        macro.key_tap("esc")
        self._wait(0.5)
        macro.key_tap("r")
        self._wait(0.5)
        macro.key_tap("enter")
        self._wait(float(mv.get("reset_wait", 3.5)))
        self._set(msg="기준 장소로 이동 · 카메라 정렬 (채팅 · 도감)")
        macro.key_tap("/")
        self._wait(0.5)
        self._click(b["chat_pos"])
        self._wait(0.5)
        self._click(b["collection_pos"])
        self._wait(0.5)
        self._click(b["collection_close"])
        self._wait(0.5)
        macro.key_tap("/")
        self._wait(0.5)
        macro.key_tap("enter")
        self._set(msg="기준 장소로 이동 · 걷기 (W → W+A)")
        self._rect()
        self._hold(("w",), float(mv.get("w_time", 0.85)))
        self._hold(("w", "a"), float(mv.get("wa_time", 8.0)))
        self._wait(0.5)
        self._set(msg="기준 장소로 이동 · 화면 내려다보기 (우클릭 드래그)")
        self._tilt_down(int(mv.get("tilt_px", 800)))
        self._wait(0.2)
        self._set(msg="기준 장소로 이동 · 최대 줌 (O)")
        self._hold(("o",), float(mv.get("o_time", 2.5)))
        self._wait(0.3)

    # ---- 2. 장소로
    def _place(self, i):
        places = (self.get_move() or {}).get("places") or []
        if not 0 <= i < len(places):
            raise RuntimeError("장소를 찾을 수 없음")
        return places[i]

    def walk(self, i, upto=None):
        """장소 i 의 지점들을 차례로 (upto 앞까지) 누르고 각 지점의 시간만큼 기다림"""
        pl, mv = self._place(i), self.get_move() or {}
        pts = pl["points"][:upto] if upto is not None else pl["points"]
        for n, pt in enumerate(pts, 1):
            if not pt.get("pos"):
                raise RuntimeError(f"{pl['name'] or '장소'} · {n}번 지점 위치가 없음")
            if pt.get("time") is None:
                raise RuntimeError(f"{pl['name'] or '장소'} · {n}번 지점 시간을 먼저 재야 함 (테스트)")
            self._set(msg=f"{pl['name'] or '장소'} 가는 중 ({n}/{len(pl['points'])})")
            self._click(pt["pos"], mv.get("button", "right"))
            self._wait(float(pt["time"]) + float(mv.get("margin", 0.3)))

    def go_place(self, i):
        pl = self._place(i)
        self.go_base()
        self.walk(i)
        self.log(f"{pl['name'] or '장소'} 도착", "g")

    # ---- 3. 시간 재기
    def measure(self, i, j, from_base=True):
        """장소 i 의 지점 j 를 눌러 놓고, 도착(F6 · 화면 버튼)할 때까지 걸린 시간을 잼
        from_base: 기준 장소부터 (아니면 지금 화면에서 바로 — 이미 기준 장소 · 앞 지점에 있을 때)"""
        pl, mv = self._place(i), self.get_move() or {}
        if not 0 <= j < len(pl["points"]) or not pl["points"][j].get("pos"):
            raise RuntimeError("지점 위치를 먼저 지정")
        if from_base:
            self.go_base()
            self.walk(i, upto=j)
        self.arrive_ev.clear()
        self._banner("measure")
        name = f"{pl['name'] or '장소'} · {j + 1}번 지점"
        self._click(pl["points"][j]["pos"], mv.get("button", "right"))
        t0 = time.time()
        self._set(msg=f"{name}까지 걷는 중 — 도착하면 F6", measuring={"place": i, "point": j, "since": t0})
        self.log(f"{name} 시간 재는 중 — 도착하면 F6 (또는 화면의 [도착])", "c")
        was = macro.key_down_now(ARRIVE_KEY)        # 이미 누르고 있던 F6 은 안 셈 (새로 누른 순간만)
        while True:
            self._check()
            down = macro.key_down_now(ARRIVE_KEY)
            if self.arrive_ev.is_set() or (down and not was):
                break
            was = down
            if time.time() - t0 > MEASURE_MAX:
                raise RuntimeError(f"{MEASURE_MAX:g}초 안에 도착을 안 누름 — 시간 재기 취소")
            time.sleep(0.01)
        sec = round(time.time() - t0, 2)
        if self.set_move_time:
            self.set_move_time(i, j, sec)
        self.log(f"{name} 걸린 시간 {sec:g}초 저장", "g")
        return sec
