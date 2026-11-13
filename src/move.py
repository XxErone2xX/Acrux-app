# -*- coding: utf-8 -*-
"""
이동 — 기준 장소에서 사용자가 직접 걸어간 키 입력을 그대로 다시 눌러 원하는 장소로 감 (TinyTask 처럼 · Click to Move 는 안 씀)
외부 앱이라 게임 안 좌표를 모르므로, 매번 '같은 자리 · 같은 화면'을 만든 뒤 같은 키를 같은 시간에 누름:
  1. 기준 장소: 리셋(Esc → R → Enter) → / · 채팅 · 도감 열고 닫기 · / · Enter (카메라 정렬)
     → W 1초 → W+A 7초 → A 1초 (0.75초 뒤부터 W 0.25초 같이) (구석으로 걸어가 항상 같은 자리) → 우클릭 드래그(위에서 내려다보기) → O 2.5초 (최대 줌)
  2. 길 기록: 기준 장소에 도착하면 사용자가 W · A · S · D · Space (+ 로블록스 창 안 왼쪽 클릭) 로 걸어감
     → 처음 키를 누른 순간부터 도착(F6 또는 화면의 [도착])까지 누르고 뗀 시각을 저장
  3. 이동: 기준 장소 → 저장한 입력을 같은 시각에 그대로 누르고 뗌
"""
import threading
import time

import macro

ARRIVE_KEY = "f6"        # 시간 잴 때 도착하면 누르는 키
MEASURE_MAX = 180.0      # 시간 재기 최대 (초)
MOVE_KEYS = ("w", "a", "s", "d", "o", "space", "shift", "e")
REC_KEYS = ("w", "a", "s", "d", "space")   # 길 기록에 쓰는 키 (그 밖의 키는 기록 안 함 · E 는 안 씀)
REC_MAX = 300.0          # 길 기록 최대 (초)


class Stopped(Exception):
    pass


class Mover:
    LABEL = "이동"

    def __init__(self, get_base, get_move, log, set_route=None, before=None, after=None, banner=None, progress=None):
        self.get_base = get_base            # 통합 위치 (Collection 버튼 · 닫기 위치)
        self.get_move = get_move            # 이동 설정 (move)
        self.log = log
        self.set_route = set_route          # (장소, 길) → 기록한 길 저장
        self.before, self.after = before, after   # 자동 낚시 잠깐 멈춤 / 이어감
        self.banner = banner                # 화면 위 안내 띠: banner('move' · 'record' · None)
        self.progress = progress            # 안내 띠 게이지: progress(목표 0~1, 걸리는 초)
        self._plan, self._acc = 0.0, 0.0    # 이번 이동 전체 예상 시간 · 지금까지 지난 예상 시간
        self.stop_ev = threading.Event()
        self.arrive_ev = threading.Event()
        self.thread = None
        self.lock = threading.Lock()
        self.state = {"msg": "대기", "measuring": None, "error": None}

    # ---- 상태
    def running(self):
        """이동이 도는 중 (이 스레드 · 또는 판매처럼 다른 곳에서 이동을 빌려 쓰는 중)"""
        return bool(self.thread and self.thread.is_alive()) or bool(self.state.get("ext"))

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
        """job: 'base' (기준 장소로만) / 'rev' (반대쪽 기준 장소) / 'place' (장소 i 로 이동) / 'record' (장소 i 길 기록)"""
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
                if self.before() is False:
                    self.log("이동 취소 — 자동 낚시가 멈추지 않음", "y")
                    return
            if job == "base":
                self.plan(self.base_time())
            elif job == "rev":
                self.plan(self.rev_time())
            elif job == "place":
                self.plan(self.start_time(args[0]) + self.place_time(args[0]))
            elif job == "record":
                self.plan(self.start_time(args[0]))
            else:
                self.plan(0)
            self._banner("move")
            if job == "base":
                self.go_base()
                self.log("기준 장소 도착", "g")
            elif job == "rev":
                self.go_base_rev()
                self.log("반대쪽 기준 장소 도착", "g")
            elif job == "place":
                self.go_place(*args)
            elif job == "record":
                self.record(*args)
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

    # ---- 게이지 (안내 띠가 예상 시간에 맞춰 차오름)
    def base_time(self):
        """기준 장소까지 예상 시간 (go_base 순서 · 화면의 총 시간과 같은 계산)"""
        m = self.get_move() or {}
        return 4.9 + float(m.get("reset_wait", 3.5)) + float(m.get("w_time", 1.0)) + float(m.get("wa_time", 7.0)) \
            + max(float(m.get("a_time", 1.0)), float(m.get("w2_time", 0.25))) + float(m.get("sd_time", 0.25)) \
            + max(float(m.get("o_time", 2.5)), float(m.get("tilt_px", 800)) / 20 * 0.015)

    def rev_time(self):
        """반대쪽 기준 장소까지 예상 시간 (go_base_rev 순서)"""
        m = self.get_move() or {}
        return 4.4 + float(m.get("reset_wait", 3.5)) + max(float(m.get("o_time", 2.5)), float(m.get("tilt_px", 800)) / 20 * 0.015) \
            + float(m.get("rsd_time", 1.0)) + max(float(m.get("rs_time", 6.0)), float(m.get("ra_time", 1.0)))

    def quest_time(self):
        """퀘스트 보드 동작(E → 대기 → Exit → D) + 마지막 D"""
        m = self.get_move() or {}
        return 0.6 + float(m.get("q_wait", 1.5)) + float(m.get("qd_time", 2.0)) + float(m.get("qd_wait", 0.65)) + float(m.get("rd_time", 2.0))

    def is_rev(self, i):
        """반대쪽 기준 장소에서 출발하는 장소 (포션 제작 장소 — 낚시와 정반대 방향)"""
        try:
            return self._place(i).get("feat") == "mcraft"
        except RuntimeError:
            return False

    def start_time(self, i):
        return self.rev_time() if self.is_rev(i) else self.base_time()

    def place_time(self, i):
        """기록한 길 길이 + 도착 여유"""
        m = self.get_move() or {}
        try:
            r = self._place(i).get("route") or {}
        except RuntimeError:
            return 0.0
        return float(r.get("len") or 0) + float(m.get("margin", 0.3)) if r.get("ev") else 0.0

    def plan(self, total):
        """이번 이동 전체 예상 시간 (0 = 게이지 안 씀)"""
        self._plan, self._acc = float(total), 0.0

    def _advance(self, sec, cap=1.0):
        """다음 sec 초 동안 게이지를 그만큼 채움"""
        if self._plan <= 0 or not self.progress:
            return
        self._acc += sec
        try:
            self.progress(min(cap, self._acc / self._plan), sec)
        except Exception:
            pass

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
        # 이동은 키 입력이 들어가야 함 → 로블록스가 맨 앞이 아니면 앞으로 가져오고 제목 줄을 한 번 눌러 입력을 받게 함
        if macro.focus(hwnd, wait=0.15, click=True):
            self.log("로블록스 창을 맨 앞으로 가져옴", "d")
        if not macro.is_foreground(hwnd):
            raise RuntimeError("로블록스 창을 맨 앞으로 가져오지 못함 — 로블록스 창을 한 번 눌러 주세요")
        return rect

    def _click(self, pos, button="left"):
        if button == (self.get_move() or {}).get("button", "right"):
            macro.mark_moved()                   # 갈 곳 클릭 (Click to Move)
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
        return f"먼저 지정 필요: {', '.join(miss)} (매크로 설정 → 공통 위치 → 이동 · 기준 장소)" if miss else None

    def path_missing(self, i):
        """장소 i 의 길이 기록돼 있나 → 안내 글 또는 None"""
        places = (self.get_move() or {}).get("places") or []
        if not 0 <= i < len(places):
            return "장소를 찾을 수 없음"
        pl = places[i]
        if not (pl.get("route") or {}).get("ev"):
            return f"{pl['name'] or '장소'} 길을 먼저 기록해야 함 (매크로 설정 → 장소 → [길 기록])"
        return None

    # ---- 1. 기준 장소 (사용자가 정한 순서)
    def _hold_with(self, key, sec, extra, extra_sec):
        """key 를 sec 초 동안 계속 누르고, 끝나기 extra_sec 초 전부터 extra 도 같이 누름 (key 는 중간에 안 뗌)"""
        macro.mark_moved()
        sec, extra_sec = max(0.0, float(sec)), max(0.0, float(extra_sec))
        start = time.time()
        end, extra_at = start + max(sec, extra_sec), start + max(0.0, sec - extra_sec)
        keys = [key]
        try:
            macro.key_down(key)
            while time.time() < end:
                self._check()
                if extra not in keys and time.time() >= extra_at:
                    keys.append(extra)
                    macro.key_down(extra)
                time.sleep(min(0.033, max(0.0, min(end, extra_at if extra not in keys else end) - time.time())))
                if time.time() < end:
                    for k in keys:
                        macro.key_down(k)          # 자동 반복 (A 는 계속 눌린 채로)
        finally:
            for k in reversed(keys):
                macro.key_up(k)

    def _hold(self, keys, sec):
        """키들을 sec 초 동안 누르고 있다가 뗌 (멈추면 바로 뗌)
        진짜 키보드처럼 누르는 동안 '누름' 신호를 계속 다시 보냄 (약 30번/초) — 한 번만 보내면
        누를 때마다 한 칸씩 움직이는 것(O 키 줌 · 화면 기울이기 등)은 한 칸만 움직이고 끝났음"""
        if any(k in ("w", "a", "s", "d", "space") for k in keys) and sec > 0:
            macro.mark_moved()                   # 캐릭터가 움직임 → 다음 리셋은 해야 함
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

    def _tilt_and_zoom(self, px, sec):
        """O 키(최대 줌)를 누르고 있는 동안 동시에 우클릭 드래그로 화면을 내려다보게 함
        O 는 sec 초 동안 (진짜 키보드처럼 자동 반복) · 드래그는 20px 씩 10ms 마다, 다 끌면 우클릭을 뗌"""
        rect = self._rect()
        macro.move_to(rect[0] + rect[2] // 2, rect[1] + rect[3] // 2)
        self._wait(0.1)
        dragging = px > 0
        try:
            macro.key_down("o")
            if dragging:
                macro.mouse_button("right", True)
            start = time.time()
            end = start + max(0.0, sec)
            next_key, done = start + 0.033, 0
            while True:
                self._check()
                now = time.time()
                if dragging:
                    if done < px:
                        step = min(20, px - done)
                        macro.move_rel(0, step)
                        done += step
                    else:
                        macro.mouse_button("right", False)
                        dragging = False
                if now >= end and not dragging:
                    break
                if now >= next_key and now < end:
                    macro.key_down("o")         # 자동 반복
                    next_key = now + 0.033
                time.sleep(0.01)
        finally:
            if dragging:
                macro.mouse_button("right", False)
            macro.key_up("o")

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

    def align_camera(self):
        """/ → 채팅 버튼 → 도감 버튼 → 도감 Exit → / → Enter (각 0.5초) — 도감을 열고 닫으면 카메라가 캐릭터 뒤로 맞춰짐
        (채팅창이 도감 Exit 를 가릴 때가 있어서 앞뒤로 채팅 버튼을 눌러 채팅창을 접었다 폄)"""
        b = self.get_base() or {}
        miss = [n for k, n in self.BASE_KEYS if not b.get(k)]
        if miss:
            raise RuntimeError(f"공통 위치 지정 필요: {', '.join(miss)}")
        self._rect()
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
        self._wait(0.3)

    def go_base(self):
        """Esc → R → Enter (리셋 · 0.5초 간격) → 3.5초 → / → 채팅 버튼 → 도감 버튼 → 도감 Exit → / → Enter (각 0.5초)
        → W 1초 → W+A 7초 → A 1초 (0.75초 뒤부터 W 0.25초 같이) → S+D 0.25초 → 0.5초 → O 2.5초 (최대 줌) 누르는 동안 동시에 우클릭 드래그로 위에서 내려다보기"""
        mv = self.get_move() or {}
        miss = self.base_missing()
        if miss:
            raise RuntimeError(miss)
        self._set(msg="기준 장소로 이동 · 리셋")
        self._advance(self.base_time())
        self._reset(mv)
        self._set(msg="기준 장소로 이동 · 카메라 정렬 (채팅 · 도감)")
        self.align_camera()
        self._set(msg="기준 장소로 이동 · 걷기 (W → W+A → A → W → S+D)")
        self._rect()
        self._hold(("w",), float(mv.get("w_time", 1.0)))
        self._hold(("w", "a"), float(mv.get("wa_time", 7.0)))
        # A 를 a_time 동안 누르고, A 를 누르기 시작한 지 (a_time - w2_time) 뒤부터 끝까지 W 도 같이 (기본: A 1초 · 0.75초 뒤 W 0.25초)
        self._hold_with("a", float(mv.get("a_time", 1.0)), "w", float(mv.get("w2_time", 0.25)))
        self._hold(("s", "d"), float(mv.get("sd_time", 0.25)))     # 그 다음 S + D 같이
        self._wait(0.5)
        self._set(msg="기준 장소로 이동 · 내려다보기 + 최대 줌 (우클릭 드래그 + O)")
        self._tilt_and_zoom(int(mv.get("tilt_px", 800)), float(mv.get("o_time", 2.5)))
        self._wait(0.3)

    def _reset(self, mv):
        self._release_keys()
        self._rect()
        self._wait(0.2)
        if macro.reset_fresh():                  # 방금 리셋하고 안 움직였음 → 또 리셋하지 않음 (리셋 쿨타임)
            self.log("방금 리셋함 — 리셋 건너뜀", "d")
        else:
            for k in ("esc", "r", "enter"):
                macro.key_tap(k)
                self._wait(0.5)
            macro.mark_reset()
            self._wait(max(0.0, float(mv.get("reset_wait", 3.5)) - 0.5))

    def go_base_rev(self):
        """반대쪽 기준 장소 (포션 제작처럼 낚시와 정반대 방향으로 갈 때)
        리셋 → 카메라 정렬(채팅 · 도감) → 내려다보기 + 최대 줌 → S+D 1초 → S 6초 (5초 뒤부터 A 1초 같이)"""
        mv = self.get_move() or {}
        miss = self.base_missing()
        if miss:
            raise RuntimeError(miss)
        self._set(msg="반대쪽 기준 장소로 이동 · 리셋")
        self._advance(self.rev_time())
        self._reset(mv)
        self._set(msg="반대쪽 기준 장소로 이동 · 카메라 정렬 (채팅 · 도감)")
        self.align_camera()
        self._set(msg="반대쪽 기준 장소로 이동 · 내려다보기 + 최대 줌 (우클릭 드래그 + O)")
        self._tilt_and_zoom(int(mv.get("tilt_px", 800)), float(mv.get("o_time", 2.5)))
        self._wait(0.3)
        self._set(msg="반대쪽 기준 장소로 이동 · 걷기 (S+D → S → S+A)")
        self._rect()
        self._hold(("s", "d"), float(mv.get("rsd_time", 1.0)))
        # S 를 rs_time 동안 누르고, 끝나기 ra_time 초 전부터 A 도 같이 (기본: S 6초 · 5초 뒤 A 1초)
        self._hold_with("s", float(mv.get("rs_time", 6.0)), "a", float(mv.get("ra_time", 1.0)))
        self._wait(0.5)

    def go_start(self, i):
        """장소 i 의 출발점: 포션 제작 장소는 반대쪽 기준 장소 · 나머지는 기준 장소"""
        if self.is_rev(i):
            self.go_base_rev()
        else:
            self.go_base()

    # ---- 2. 장소로
    def _place(self, i):
        places = (self.get_move() or {}).get("places") or []
        if not 0 <= i < len(places):
            raise RuntimeError("장소를 찾을 수 없음")
        return places[i]

    def _until(self, t):
        """perf_counter 시각 t 까지 기다림 (F7 확인 · 끝 무렵은 촘촘히)"""
        while True:
            self._check()
            left = t - time.perf_counter()
            if left <= 0:
                return
            time.sleep(min(0.01, left) if left > 0.002 else 0)

    def walk(self, i):
        """장소 i 의 기록한 길을 그대로 누름 (기준 장소에 서 있다고 봄) — 키를 같은 시각에 누르고 떼고 · 클릭은 같은 자리"""
        pl, mv = self._place(i), self.get_move() or {}
        r = pl.get("route") or {}
        ev = r.get("ev") or []
        if not ev:
            raise RuntimeError(f"{pl['name'] or '장소'} 길을 먼저 기록해야 함 (매크로 설정 → 장소 → [길 기록])")
        name = pl["name"] or "장소"
        self._set(msg=f"{name} 가는 중 (기록한 길 · {float(r.get('len') or 0):g}초)")
        self._advance(self.place_time(i))
        self._rect()
        macro.mark_moved()                         # 캐릭터가 움직임 → 다음 리셋은 해야 함
        down = set()
        t0 = time.perf_counter()
        try:
            for t, k, v in ev:
                self._until(t0 + float(t))
                if k == "click":
                    self._click(v, "left")
                elif v:
                    macro.key_down(k)
                    down.add(k)
                else:
                    macro.key_up(k)
                    down.discard(k)
            self._until(t0 + float(r.get("len") or 0))
        finally:
            for k in down:
                macro.key_up(k)
        self._wait(float(mv.get("margin", 0.3)))

    def go_place(self, i):
        pl = self._place(i)
        self.go_start(i)
        self.walk(i)
        self.log(f"{pl['name'] or '장소'} 도착", "g")

    # ---- 3. 길 기록
    def record(self, i):
        """기준 장소(또는 반대쪽 기준 장소)로 간 뒤, 사용자가 걸어가는 키 입력을 기록 → 도착(F6 · 화면의 [도착])하면 저장
        W · A · S · D · Space 를 누르고 뗀 시각 + 로블록스 창 안 왼쪽 클릭 자리 · 처음 키를 누른 순간이 0초"""
        pl = self._place(i)
        name = pl["name"] or "장소"
        self.go_start(i)
        self.arrive_ev.clear()
        self._banner("record")
        rect = self._rect()
        self._set(msg=f"{name} 길 기록 중 — 걸어가서 도착하면 F6", measuring={"place": i, "rec": True, "since": time.time()})
        self.log(f"{name} 길 기록 중 — WASD · Space 로 걸어가고 도착하면 F6 (또는 화면의 [도착]) · 화면은 돌리지 마세요", "c")
        held = {k: macro.key_down_now(k) for k in REC_KEYS}     # 이미 누르고 있던 키는 뗀 뒤부터
        lb = macro.mouse_down_now("left")
        f6 = macro.key_down_now(ARRIVE_KEY)
        ev, t0, start = [], None, time.time()
        while True:
            self._check()
            now = time.perf_counter()
            for k in REC_KEYS:
                d = macro.key_down_now(k)
                if d == held[k]:
                    continue
                held[k] = d
                if t0 is None:
                    if not d:
                        continue
                    t0 = now
                ev.append([round(now - t0, 3), k, 1 if d else 0])
            d = macro.mouse_down_now("left")
            if d != lb:
                lb = d
                x, y = macro.cursor_pos()
                if d and rect[0] <= x < rect[0] + rect[2] and rect[1] <= y < rect[1] + rect[3]:   # 로블록스 창 안 클릭만
                    if t0 is None:
                        t0 = now
                    ev.append([round(now - t0, 3), "click", list(macro.to_ratio(x, y, rect))])
            f = macro.key_down_now(ARRIVE_KEY)
            if self.arrive_ev.is_set() or (f and not f6):
                break
            f6 = f
            if time.time() - start > REC_MAX:
                raise RuntimeError(f"{REC_MAX:g}초 안에 도착을 안 누름 — 길 기록 취소")
            time.sleep(0.004)
        if t0 is None or not ev:
            raise RuntimeError("아무 키도 안 눌러서 길을 저장하지 않음")
        end = round(time.perf_counter() - t0, 3)
        for k in REC_KEYS:                          # 도착할 때 아직 누르고 있던 키는 끝에서 뗌
            if held[k] and any(e[1] == k for e in ev):
                ev.append([end, k, 0])
        route = {"ev": ev, "len": round(end, 2)}
        if self.set_route:
            self.set_route(i, route)
        self.log(f"{name} 길 저장 ({round(end, 1):g}초 · 입력 {len(ev)}개)", "g")
        return route
