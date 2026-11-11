# -*- coding: utf-8 -*-
"""
모드 · 플러그인 — Acrux 본체에 없는 기능을 따로 만들어 넣는 확장
  모드 (type: "mod")      설정 탭 · 화면이 있는 큰 기능
  플러그인 (type: "plugin") 화면 없이 뒤에서 이벤트에 반응하는 작은 기능 (예: 바이옴이 바뀌면 소리)

설치 위치: <데이터 폴더>/mods/<id>/  (config.json 옆 · 업데이트로 앱 폴더를 갈아도 남음)
  mod.json    {"id": "my_mod", "name": "내 모드", "version": "1.0.0", "type": "mod", "author": "…",
               "description": "…", "entry": "main", "acrux": "3.0.0",
               "ui": {"html": "ui.html", "js": "ui.js", "css": "ui.css"}}
  main.py     setup(ctx) 함수 — 켤 때 한 번 불림 (ctx = Context: 로그 · 설정 · API 등록 · 이벤트)
  ui.*        (모드만) 화면에 붙는 HTML · JS · CSS — /mods/<id>/<파일> 로 열림

모드 파일 (.acxm) = 위 폴더를 zip 으로 묶은 것 (mod.json 이 맨 위나 폴더 하나 안에)
모드는 파이썬 코드라 Acrux 와 같은 권한으로 돎 → 믿을 수 있는 곳에서 받은 것만 설치
켜기 · 끄기 · 설치 · 삭제는 Acrux 를 다시 켜면 적용
"""
import importlib
import json
import re
import shutil
import sys
import threading
import traceback
import types
import zipfile
from pathlib import Path

import core

MOD_DIR = core.DATA_BASE / "mods"
PKG = "acrux_mods"                     # 모드는 acrux_mods.<id> 패키지로 불러옴 (모드끼리 · 본체와 이름이 안 겹치게)
ID_RE = re.compile(r"[a-z][a-z0-9_]{1,30}")
TYPES = ("mod", "plugin")
UI_EXT = {".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8",
          ".png": "image/png", ".gif": "image/gif", ".svg": "image/svg+xml", ".json": "application/json; charset=utf-8"}
MAX_ZIP = 30 * 1024 * 1024             # 모드 파일 최대 크기 (압축 풀었을 때)


# ---------------------------------------------------------------- 이벤트
class Hooks:
    """본체가 부르는 이벤트 → 모드가 등록한 함수들
      busy(kind)               모드 일이 도는 중이면 True → 매크로가 쉼 · 로블록스 꺼짐 알림 안 띄움
                               kind: "macro" (매크로 루프) · "crash" (꺼짐 감시) · "afk" (AFK 점프) · "mpop" (내 서버 팝핑)
      active()                 모드가 켜져 있음 (로블록스 꺼짐 감시 · AFK 점프 대상) → True
      state(out) · poll(out)   화면에 보낼 값에 더하기 (out 을 고침)
      stats()                  통계 (이번 실행) {키: 숫자}
      ingame(path, reason)     Play 클릭으로 게임에 들어감 (reason 이 '복귀' 가 아닐 때)
      play_fail(reason)        Play 클릭으로 못 들어감 (reason 이 '복귀' 가 아닐 때)
      after_return(head, wait) 내 서버로 복귀 끝 (로블록스 꺼짐 복구 말고) → True 를 돌려주면 본체는 아무것도 안 함
      biome(prev, found, away)  바이옴이 바뀜 (away: 모드가 들어간 다른 사람 서버인지)
      macro(on)                매크로 버튼이 켜지거나 꺼짐
      stop()                   [정지] · F7 — 하던 일 멈춤
      ready()                  창이 뜨기 직전 (모든 모드를 불러온 뒤)
      shutdown()               Acrux 를 끔"""

    def __init__(self, log):
        self.fns, self.log = {}, log

    def on(self, event, fn):
        self.fns.setdefault(event, []).append(fn)

    def emit(self, event, *args):
        out = []
        for fn in list(self.fns.get(event, ())):
            try:
                out.append(fn(*args))
            except Exception as e:
                out.append(None)
                self.log(f"모드 오류 ({event}): {type(e).__name__}: {e}", "r")
                _crash(f"mod hook {event}: {traceback.format_exc()}")
        return out

    def any(self, event, *args):
        return any(bool(x) for x in self.emit(event, *args))


def _crash(msg):
    try:
        with open(core.DATA_BASE / "crash.log", "a", encoding="utf-8") as f:
            f.write(msg + "\n")
    except Exception:
        pass


# ---------------------------------------------------------------- 모드가 쓰는 창구
class Context:
    """모드 · 플러그인의 setup(ctx) 에 넘어가는 것
      ctx.id · ctx.dir · ctx.info (mod.json) · ctx.bridge (본체 · 고급) · ctx.core (설정 · 로그 · 로블록스 링크 열기)
      ctx.log(글, 색)              로그 (색: g 초록 · y 노랑 · r 빨강 · c 하늘 · d 흐림)
      ctx.api(이름)(함수)          화면에서 api('이름', {...}) 로 부르는 함수 (이름: 영문 소문자 · _ · 다른 것과 안 겹치게)
      ctx.on(이벤트, 함수)         Hooks 설명 참고
      ctx.config_keys(기본값, 정리) config.json 맨 위에 쓸 키 · 기본값 dict · 정리 함수 (d → d)
      ctx.settings / ctx.save()    모드 자기 설정 (config.json → mod_settings.<id>) · 저장"""

    def __init__(self, bridge, mod):
        self.bridge, self.mod, self.core = bridge, mod, core
        self.id, self.dir, self.info = mod.id, mod.dir, mod.info

    def log(self, msg, color=""):
        self.bridge._on_log(msg, color)

    def api(self, name):
        if not re.fullmatch(r"[a-z_]{1,40}", name or ""):
            raise ValueError(f"API 이름은 영문 소문자 · _ 만: {name}")

        def reg(fn):
            if hasattr(self.bridge, "api_" + name) and name not in self.bridge.mod_apis:
                raise ValueError(f"본체 API 와 이름이 겹침: {name}")
            if self.bridge.mod_apis.get(name, self.id) != self.id:
                raise ValueError(f"다른 모드 API 와 이름이 겹침: {name}")
            self.bridge.mod_apis[name] = self.id
            setattr(self.bridge, "api_" + name, fn)
            return fn
        return reg

    def on(self, event, fn):
        self.bridge.hooks.on(event, fn)

    def config_keys(self, defaults, normalize=None, locked=()):
        """모드가 쓰는 config.json 맨 위 키 (defaults: 처음 값) · normalize(d): 정리 · locked: 화면이 못 바꾸는 키 (모드 엔진만 씀)"""
        self.bridge.mod_keys |= set(defaults)
        self.bridge.locked_keys |= set(locked)

        def norm(d):
            for k, v in defaults.items():
                if k not in d:
                    d[k] = json.loads(json.dumps(v))
            return normalize(d) if normalize else d
        core.NORMALIZERS.append(norm)
        with self.bridge.lock:
            self.bridge.data = core.normalize(self.bridge.data)
        self.bridge.cfg.set_data(dict(self.bridge.data))

    @property
    def settings(self):
        return self.bridge.data.setdefault("mod_settings", {}).setdefault(self.id, {})

    def save(self):
        self.bridge._save()


# ---------------------------------------------------------------- 설치된 모드
class Mod:
    def __init__(self, path):
        self.dir = Path(path)
        self.info = json.loads((self.dir / "mod.json").read_text("utf-8-sig"))
        self.id = str(self.info.get("id") or "")
        if not ID_RE.fullmatch(self.id) or self.id != self.dir.name:
            raise ValueError(f"모드 ID 가 올바르지 않음: {self.id!r} (폴더 이름과 같아야 함 · 영문 소문자로 시작)")
        self.type = self.info.get("type") if self.info.get("type") in TYPES else "plugin"
        self.loaded, self.error = False, ""

    def ui_files(self):
        ui = self.info.get("ui") if self.type == "mod" and isinstance(self.info.get("ui"), dict) else {}
        out = {}
        for k in ("html", "js", "css"):
            f = str(ui.get(k) or "")
            if f and (self.dir / f).is_file() and Path(f).suffix in UI_EXT:
                out[k] = f"/mods/{self.id}/{f}"
        return out

    def public(self, enabled):
        i = self.info
        return {"id": self.id, "name": str(i.get("name") or self.id), "version": str(i.get("version") or ""),
                "type": self.type, "author": str(i.get("author") or ""), "description": str(i.get("description") or ""),
                "enabled": enabled, "loaded": self.loaded, "error": self.error,
                "ui": self.ui_files() if self.loaded else {}}


def scan():
    out = []
    if not MOD_DIR.is_dir():
        return out
    for d in sorted(MOD_DIR.iterdir()):
        if d.is_dir() and (d / "mod.json").is_file():
            try:
                out.append(Mod(d))
            except Exception as e:
                core.log(f"모드 폴더를 못 읽음: {d.name} — {e}", "r")
    return out


def _package():
    if PKG not in sys.modules:
        pkg = types.ModuleType(PKG)
        pkg.__path__ = [str(MOD_DIR)]
        sys.modules[PKG] = pkg
    return sys.modules[PKG]


def load_all(bridge):
    """켜진 모드를 전부 불러옴 (Bridge 를 만든 뒤 · 창을 띄우기 전)"""
    bridge.mods = scan()
    MOD_DIR.mkdir(parents=True, exist_ok=True)
    _package()
    states = bridge.data.get("mods") or {}
    for m in bridge.mods:
        if not (states.get(m.id) or {}).get("enabled", True):
            continue
        try:
            entry = str(m.info.get("entry") or "main")
            if not re.fullmatch(r"[a-z_][a-z0-9_]*", entry):
                raise ValueError("entry 가 올바르지 않음")
            module = importlib.import_module(f"{PKG}.{m.id}.{entry}")
            if not hasattr(module, "setup"):
                raise ValueError("setup(ctx) 함수가 없음")
            module.setup(Context(bridge, m))
            m.loaded = True
            bridge._on_log(f"{'모드' if m.type == 'mod' else '플러그인'} 켜짐: {m.info.get('name') or m.id} "
                           f"V{m.info.get('version') or '?'}", "c")
        except Exception as e:
            m.error = f"{type(e).__name__}: {e}"
            bridge._on_log(f"모드를 못 켬: {m.id} — {m.error}", "r")
            _crash(f"mod load {m.id}: {traceback.format_exc()}")


def listing(bridge):
    states = bridge.data.get("mods") or {}
    return [m.public(bool((states.get(m.id) or {}).get("enabled", True))) for m in getattr(bridge, "mods", [])]


# ---------------------------------------------------------------- 설치 · 삭제
def _read_info(zf):
    """zip 안의 mod.json 위치 (맨 위 또는 폴더 하나 안) → (앞 경로, 정보)"""
    names = [n for n in zf.namelist() if n.endswith("mod.json") and n.count("/") <= 1]
    if not names:
        raise ValueError("mod.json 이 없음 — Acrux 모드 파일이 아님")
    n = min(names, key=len)
    return n[:-len("mod.json")], json.loads(zf.read(n).decode("utf-8-sig"))


def install(path):
    """모드 파일 (.acxm · 예전 .acrux · .zip) 설치 → 정보 dict (같은 ID 가 있으면 덮어씀)"""
    path = Path(path)
    if not path.is_file():
        raise ValueError("파일이 없음")
    with zipfile.ZipFile(path) as zf:
        prefix, info = _read_info(zf)
        mid = str(info.get("id") or "")
        if not ID_RE.fullmatch(mid):
            raise ValueError(f"모드 ID 가 올바르지 않음: {mid!r}")
        total = sum(i.file_size for i in zf.infolist())
        if total > MAX_ZIP:
            raise ValueError("모드 파일이 너무 큼")
        MOD_DIR.mkdir(parents=True, exist_ok=True)
        tmp = MOD_DIR / f".{mid}.tmp"
        shutil.rmtree(tmp, ignore_errors=True)
        tmp.mkdir()
        root = tmp.resolve()
        for i in zf.infolist():
            if not i.filename.startswith(prefix) or i.is_dir():
                continue
            rel = i.filename[len(prefix):]
            dest = (tmp / rel).resolve()
            if root not in dest.parents:              # ../ 로 폴더 밖에 쓰는 파일은 거부
                shutil.rmtree(tmp, ignore_errors=True)
                raise ValueError(f"잘못된 경로가 들어 있음: {i.filename}")
            dest.parent.mkdir(parents=True, exist_ok=True)
            with zf.open(i) as src, open(dest, "wb") as out:
                shutil.copyfileobj(src, out)
    target = MOD_DIR / mid
    old = MOD_DIR / f".{mid}.old"
    shutil.rmtree(old, ignore_errors=True)
    if target.exists():
        target.rename(old)
    tmp.rename(target)
    shutil.rmtree(old, ignore_errors=True)
    Mod(target)                                       # 형식 확인
    return info


def remove(mid):
    if not ID_RE.fullmatch(str(mid or "")):
        raise ValueError("모드 ID 가 올바르지 않음")
    target = MOD_DIR / mid
    if not target.is_dir():
        raise ValueError("설치되지 않은 모드")
    shutil.rmtree(target)


def serve(path):
    """/mods/<id>/<파일> → (내용, 형식) · 없으면 None (켜진 모드의 화면 파일만)"""
    m = re.fullmatch(r"/mods/([a-z][a-z0-9_]{1,30})/([A-Za-z0-9_./-]{1,120})", path)
    if not m:
        return None
    base = (MOD_DIR / m.group(1)).resolve()
    f = (base / m.group(2)).resolve()
    if base not in f.parents or not f.is_file() or f.suffix not in UI_EXT:
        return None
    return f.read_bytes(), UI_EXT[f.suffix]


_lock = threading.Lock()
