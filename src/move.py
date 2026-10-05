# -*- coding: utf-8 -*-
"""
이동 — 기준 장소에서 화면의 한 점을 눌러(Click to Move) 원하는 장소로 걸어감
외부 앱이라 게임 안 좌표를 모르므로, 매번 '같은 화면'을 만든 뒤 그 화면의 같은 점을 누름:
  1. 기준 장소: 리셋(Esc → R → Enter) → 다시 생기면 카메라 정렬(Collection 열고 닫기 = 캐릭터 뒤 기본 방향)
     → 줌(휠을 끝까지 당긴 뒤 정해진 만큼 밀기 = 항상 같은 거리)   (FishSol 의 기준 장소 방식)
  2. 장소마다 지점 목록: [누를 곳, 걸리는 시간] — 지점이 여러 개면 앞 지점에 도착한 화면에서 다음 지점을 누름
  3. 걸리는 시간은 직접 잼: 누른 순간부터 사용자가 '도착'(F6 또는 화면의 버튼)을 누를 때까지
"""
import threading
import time

import macro

ARRIVE_KEY = "f6"        # 시간 잴 때 도착하면 누르는 키
MEASURE_MAX = 180.0      # 시간 재기 최대 (초)
MOVE_KEYS = ("w", "a", "s", "d", "space", "shift", "e")


class Stopped(Exception):
    pass


class Mover:
    LABEL = "이동"

    def __init__(self, get_base, get_move, log, set_move_time=None, before=None, after=None):
        self.get_base = get_base            # 통합 위치 (Collection 버튼 · 닫기 위치)
        self.get_move = get_move            # 이동 설정 (move)
        self.log = log
        self.set_move_time = set_move_time  # (장소, 지점, 초) → 잰 시간 저장
        self.before, self.after = before, after   # 자동 낚시 잠깐 멈춤 / 이어감
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
            if job == "base":
                self.go_base()
                self.log("기준 장소 도착 (리셋 · 카메라 정렬 · 줌 완료)", "g")
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
            self.log(f"이동 오류: {e}", "r")
        finally:
            self._release_keys()
            self._set(msg="대기", measuring=None)
            if held and self.after:
                self.after()

    # ---- 도우미
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

    # ---- 1. 기준 장소
    def go_base(self):
        b, mv = self.get_base() or {}, self.get_move() or {}
        if not b.get("collection_pos") or not b.get("collection_close"):
            raise RuntimeError("통합 위치 → 이동 에서 Collection 버튼 · 닫기 위치를 먼저 지정")
        self._set(msg="기준 장소로 이동 · 리셋")
        self._release_keys()
        rect = self._rect()
        self._wait(0.2)
        macro.key_tap("esc")
        self._wait(0.5)
        macro.key_tap("r")
        self._wait(0.5)
        macro.key_tap("enter")
        self._wait(float(mv.get("reset_wait", 2.6)))
        self._set(msg="기준 장소로 이동 · 카메라 정렬")
        self._click(b["collection_pos"])
        self._wait(0.3)
        self._click(b["collection_close"])
        self._wait(0.3)
        self._set(msg="기준 장소로 이동 · 줌")
        rect = self._rect()
        macro.move_to(rect[0] + rect[2] // 2, rect[1] + rect[3] // 2)      # 휠은 마우스가 게임 화면 위에 있어야 먹음
        self._wait(0.1)
        for _ in range(int(mv.get("zoom_in", 80))):
            macro.scroll(1)
            time.sleep(0.005)
        self._wait(0.5)
        for _ in range(int(mv.get("zoom_out", 45))):
            macro.scroll(-1)
            time.sleep(0.005)
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
