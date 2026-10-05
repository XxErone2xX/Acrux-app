# -*- coding: utf-8 -*-
"""
디스코드 통신(게이트웨이) 읽기 — 디스코드 내부 코드를 뒤지지 않고, 디스코드가 서버에서 받는 데이터를 그대로 읽음

디스코드 앱은 wss://gateway.discord.gg 웹소켓으로 새 메시지(MESSAGE_CREATE)를 받음.
크롬 디버그(CDP) 의 Network 기능으로 그 웹소켓이 받은 내용을 받아서:
  1. 압축 풀기 (zlib-stream / zstd-stream — 연결 처음부터 이어서 풀어야 함)
  2. 해석 (json 또는 etf = 얼랭 바이너리 형식)
  3. MESSAGE_CREATE / MESSAGE_UPDATE → 링크 처리, READY / GUILD_CREATE 등 → 서버·채널 이름 기억
디스코드 화면 구조가 바뀌어도 통신 형식은 거의 안 바뀌어서 안정적.
"""
import base64
import json
import re
import struct
import zlib
from urllib.parse import urlparse, parse_qs

try:
    import zstandard
except Exception:                   # 런타임에 없으면 zstd 압축 연결만 못 읽음 (zlib 은 기본 내장)
    zstandard = None

ZLIB_SUFFIX = b"\x00\x00\xff\xff"
THREAD_TYPES = {10, 11, 12}
# 해석할 이벤트 (나머지는 압축만 풀고 넘어감 → 가볍게)
NAME_EVENTS = (b"READY", b"GUILD_CREATE", b"GUILD_UPDATE", b"CHANNEL_CREATE", b"CHANNEL_UPDATE",
               b"THREAD_CREATE", b"THREAD_UPDATE", b"THREAD_LIST_SYNC")


# ---------------------------------------------------------------- ETF (얼랭 바이너리 형식) 해석
class ETFError(Exception):
    pass


def etf_decode(data):
    data = memoryview(data)
    if not len(data) or data[0] != 131:
        raise ETFError("not etf")
    if data[1] == 80:              # 압축된 ETF
        size = struct.unpack(">I", data[2:6])[0]
        data = memoryview(b"\x83" + zlib.decompress(bytes(data[6:]))[:size])
    val, _ = _term(data, 1)
    return val


def _atom(name):
    if name == "nil":
        return None
    if name == "true":
        return True
    if name == "false":
        return False
    return name


def _bin(b):
    try:
        return bytes(b).decode("utf-8")
    except UnicodeDecodeError:
        return bytes(b).decode("latin-1")


def _term(d, i):
    tag = d[i]
    i += 1
    if tag == 97:                  # SMALL_INTEGER
        return d[i], i + 1
    if tag == 98:                  # INTEGER
        return struct.unpack(">i", d[i:i + 4])[0], i + 4
    if tag == 70:                  # NEW_FLOAT
        return struct.unpack(">d", d[i:i + 8])[0], i + 8
    if tag == 99:                  # FLOAT (옛 형식, 글자)
        return float(bytes(d[i:i + 31]).split(b"\x00")[0]), i + 31
    if tag in (100, 118):          # ATOM, ATOM_UTF8
        n = struct.unpack(">H", d[i:i + 2])[0]
        return _atom(_bin(d[i + 2:i + 2 + n])), i + 2 + n
    if tag in (115, 119):          # SMALL_ATOM, SMALL_ATOM_UTF8
        n = d[i]
        return _atom(_bin(d[i + 1:i + 1 + n])), i + 1 + n
    if tag in (104, 105):          # SMALL_TUPLE, LARGE_TUPLE
        if tag == 104:
            n, i = d[i], i + 1
        else:
            n, i = struct.unpack(">I", d[i:i + 4])[0], i + 4
        out = []
        for _ in range(n):
            v, i = _term(d, i)
            out.append(v)
        return out, i
    if tag == 106:                 # NIL (빈 목록)
        return [], i
    if tag == 107:                 # STRING (0~255 숫자 목록)
        n = struct.unpack(">H", d[i:i + 2])[0]
        return list(d[i + 2:i + 2 + n]), i + 2 + n
    if tag == 108:                 # LIST
        n = struct.unpack(">I", d[i:i + 4])[0]
        i += 4
        out = []
        for _ in range(n):
            v, i = _term(d, i)
            out.append(v)
        if d[i] == 106:            # 꼬리 NIL
            i += 1
        else:
            _, i = _term(d, i)
        return out, i
    if tag == 109:                 # BINARY → 글자
        n = struct.unpack(">I", d[i:i + 4])[0]
        return _bin(d[i + 4:i + 4 + n]), i + 4 + n
    if tag in (110, 111):          # SMALL_BIG, LARGE_BIG (스노우플레이크 ID 등)
        if tag == 110:
            n, i = d[i], i + 1
        else:
            n, i = struct.unpack(">I", d[i:i + 4])[0], i + 4
        sign = d[i]
        v = int.from_bytes(bytes(d[i + 1:i + 1 + n]), "little")
        return (-v if sign else v), i + 1 + n
    if tag == 116:                 # MAP
        n = struct.unpack(">I", d[i:i + 4])[0]
        i += 4
        out = {}
        for _ in range(n):
            k, i = _term(d, i)
            v, i = _term(d, i)
            if isinstance(k, list):
                k = str(k)
            out[k] = v
        return out, i
    if tag == 77:                  # BIT_BINARY
        n = struct.unpack(">I", d[i:i + 4])[0]
        return bytes(d[i + 5:i + 5 + n]), i + 5 + n
    raise ETFError(f"unknown etf tag {tag}")


# ---- 필요한 부분만 해석 (메모리 · CPU 절약)
# READY 는 내가 들어간 모든 서버의 멤버 · 역할 · 이모지 등이 통째로 들어 있어서 (서버가 많으면 수십 MB)
# 전부 파이썬 객체로 만들면 순간 메모리가 크게 튐 → 서버 · 채널 이름에 쓰는 칸만 만들고 나머지는 건너뜀
def _skip(d, i):
    """값 하나를 만들지 않고 건너뜀 → 다음 위치"""
    tag = d[i]
    i += 1
    if tag == 97:
        return i + 1
    if tag == 98:
        return i + 4
    if tag == 70:
        return i + 8
    if tag == 99:
        return i + 31
    if tag in (100, 118, 107):
        return i + 2 + struct.unpack(">H", d[i:i + 2])[0]
    if tag in (115, 119):
        return i + 1 + d[i]
    if tag in (104, 105):
        if tag == 104:
            n, i = d[i], i + 1
        else:
            n, i = struct.unpack(">I", d[i:i + 4])[0], i + 4
        for _ in range(n):
            i = _skip(d, i)
        return i
    if tag == 106:
        return i
    if tag == 108:
        n = struct.unpack(">I", d[i:i + 4])[0]
        i += 4
        for _ in range(n):
            i = _skip(d, i)
        return i + 1 if d[i] == 106 else _skip(d, i)
    if tag == 109:
        return i + 4 + struct.unpack(">I", d[i:i + 4])[0]
    if tag == 110:
        return i + 2 + d[i]
    if tag == 111:
        return i + 5 + struct.unpack(">I", d[i:i + 4])[0]
    if tag == 116:
        n = struct.unpack(">I", d[i:i + 4])[0]
        i += 4
        for _ in range(n * 2):
            i = _skip(d, i)
        return i
    if tag == 77:
        return i + 5 + struct.unpack(">I", d[i:i + 4])[0]
    raise ETFError(f"unknown etf tag {tag}")


def _pick(d, i, spec):
    """spec 대로 필요한 칸만 해석: True = 통째로 / dict = 그 키만 (값은 다시 spec) / [spec] = 목록의 각 항목"""
    if spec is True:
        return _term(d, i)
    tag = d[i]
    if isinstance(spec, dict) and tag == 116:
        n = struct.unpack(">I", d[i + 1:i + 5])[0]
        i += 5
        out = {}
        for _ in range(n):
            k, i = _term(d, i)
            if isinstance(k, list):
                k = str(k)
            sub = spec.get(k) if isinstance(k, str) else None
            if sub is None:
                i = _skip(d, i)
            else:
                out[k], i = _pick(d, i, sub)
        return out, i
    if isinstance(spec, list) and tag == 108:
        n = struct.unpack(">I", d[i + 1:i + 5])[0]
        i += 5
        out = []
        for _ in range(n):
            v, i = _pick(d, i, spec[0])
            out.append(v)
        return out, (i + 1 if d[i] == 106 else _skip(d, i))
    return _term(d, i)                 # 예상과 다른 모양이면 그냥 통째로


_CHANNEL = {"id": True, "name": True, "guild_id": True, "parent_id": True, "type": True}
_GUILD = {"id": True, "name": True, "properties": {"name": True}, "channels": [_CHANNEL], "threads": [_CHANNEL]}
# 이벤트별로 해석할 칸 — 여기 없는 이벤트는 내용(d)을 아예 안 만듦 (링크 처리 · 이름 기억에 안 씀)
EVENT_SPECS = {
    "MESSAGE_CREATE": True, "MESSAGE_UPDATE": True,
    "READY": {"guilds": [_GUILD]},
    "GUILD_CREATE": _GUILD, "GUILD_UPDATE": _GUILD,
    "CHANNEL_CREATE": _CHANNEL, "CHANNEL_UPDATE": _CHANNEL,
    "THREAD_CREATE": _CHANNEL, "THREAD_UPDATE": _CHANNEL,
    "THREAD_LIST_SYNC": {"guild_id": True, "threads": [_CHANNEL]},
}
_JSON_KEEP = {"t", "s", "op", "d", "guilds", "id", "name", "properties", "channels", "threads", "guild_id",
              "parent_id", "type"}


def etf_event(data):
    """ETF 게이트웨이 메시지 → {"op", "t", "s", "d"} — d 는 EVENT_SPECS 대로 필요한 칸만 (없는 이벤트는 d 없음)"""
    data = memoryview(data)
    if not len(data) or data[0] != 131:
        raise ETFError("not etf")
    if data[1] == 80:
        size = struct.unpack(">I", data[2:6])[0]
        data = memoryview(b"\x83" + zlib.decompress(bytes(data[6:]))[:size])
    if data[1] != 116:
        val, _ = _term(data, 1)
        return val
    n = struct.unpack(">I", data[2:6])[0]
    i, out, d_at = 6, {}, None
    for _ in range(n):
        k, i = _term(data, i)
        if k == "d":
            d_at, i = i, _skip(data, i)
        else:
            out[k], i = _term(data, i)
    spec = EVENT_SPECS.get(out.get("t"))
    if spec is True and b"roblox" not in bytes(data).lower():
        spec = None                      # 메시지인데 링크(roblox)가 없음 → 해석 안 함 (예전과 같음)
    if d_at is not None and spec is not None:
        out["d"], _ = _pick(data, d_at, spec)
    return out


def json_event(data):
    """JSON 게이트웨이 메시지 → etf_event 와 같은 모양 · 맨 앞 "t" 로 이벤트를 먼저 보고 필요 없는 건 해석 안 함
    (디스코드는 {"t":…, "s":…, "op":…, "d":…} 순서로 보냄 — 아니면 None = 예전처럼 통째로 해석)"""
    head = bytes(data[:96]) if isinstance(data, (bytes, bytearray, memoryview)) else data[:96].encode()
    m = re.match(rb'\s*\{\s*"t"\s*:\s*(?:null|"([A-Z0-9_]+)")', head)
    if not m:
        return None
    t = m.group(1).decode() if m.group(1) else None
    spec = EVENT_SPECS.get(t)
    if spec is True and b"roblox" not in (data.lower() if isinstance(data, (bytes, bytearray)) else data.encode().lower()):
        spec = None                      # 메시지인데 링크(roblox)가 없음 → 해석 안 함 (예전과 같음)
    if spec is None:
        return {"t": t, "op": 0 if t else None}     # 해석 안 함 (t 가 없으면 이벤트가 아닌 신호 — HELLO · 하트비트 등)
    if spec is True:
        return json.loads(data)
    # 이름용 이벤트: 만들어지는 객체마다 필요한 키만 남김 (멤버 목록 등은 만들자마자 버려짐)
    return json.loads(data, object_hook=lambda o: {k: v for k, v in o.items() if k in _JSON_KEEP})


# ---------------------------------------------------------------- 한 웹소켓 연결
class _Stream:
    def __init__(self, url):
        q = parse_qs(urlparse(url).query)
        self.encoding = (q.get("encoding") or ["json"])[0].lower()
        self.compress = (q.get("compress") or [""])[0].lower()
        self.buf = b""
        self.dead = False
        if self.compress == "zlib-stream":
            self.z = zlib.decompressobj()
        elif self.compress == "zstd-stream":
            if zstandard is None:
                self.dead = True
                self.z = None
            else:
                self.z = self._zstd()
        else:
            self.z = None

    @staticmethod
    def _zstd():
        d = zstandard.ZstdDecompressor()
        try:
            return d.decompressobj(read_across_frames=True)
        except TypeError:
            return d.decompressobj()

    def feed(self, raw):
        """웹소켓 메시지 하나 → 압축 푼 내용 (아직 덜 왔으면 None)"""
        if self.dead:
            return None
        if self.compress == "zlib-stream":
            self.buf += raw
            if not self.buf.endswith(ZLIB_SUFFIX):
                return None
            data, self.buf = self.buf, b""
            return self.z.decompress(data)
        if self.compress == "zstd-stream":
            return self.z.decompress(raw)
        return raw

    def event(self, data):
        """필요한 칸만 해석 (못 하면 None → decode 로 통째로)"""
        if self.encoding == "etf":
            return etf_event(data)
        return json_event(data)

    def decode(self, data):
        if self.encoding == "etf":
            return etf_decode(data)
        return json.loads(data.decode("utf-8") if isinstance(data, (bytes, bytearray)) else data)


def _s(v):
    """ID 는 글자로 통일 (ETF 는 숫자로 옴)"""
    return None if v is None else str(v)


class Gateway:
    def __init__(self):
        self.streams = {}            # CDP requestId → _Stream
        self.guilds = {}             # 서버 ID → 이름
        self.channels = {}           # 채널 ID → {name, guild, parent, type}
        self.ready = False           # READY 를 한 번이라도 해석했는지 (= 통신 읽기 정상)
        self.errors = 0
        self.last_error = ""

    # ---- CDP Network 이벤트
    @staticmethod
    def is_gateway(url):
        try:
            u = urlparse(url)
        except Exception:
            return False
        host = (u.hostname or "").lower()
        return u.scheme in ("wss", "ws") and host.startswith("gateway") and host.endswith("discord.gg")

    def created(self, request_id, url):
        if not self.is_gateway(url):
            return None
        s = _Stream(url)
        self.streams[request_id] = s
        return s

    def closed(self, request_id):
        self.streams.pop(request_id, None)

    def frame(self, request_id, opcode, payload):
        """받은 웹소켓 메시지 하나 → [(이벤트 이름, 데이터), …]"""
        s = self.streams.get(request_id)
        if not s:
            return []
        try:
            raw = payload.encode("utf-8") if opcode == 1 else base64.b64decode(payload)
            data = s.feed(raw)
        except Exception as e:           # 압축이 꼬이면 이 연결은 포기 (다음 연결부터 다시)
            s.dead = True
            self.errors += 1
            self.last_error = f"{type(e).__name__}: {e}"
            return []
        if not data:
            return []
        try:
            msg = s.event(data)
        except Exception as e:
            self.errors += 1
            self.last_error = f"{type(e).__name__}: {e}"
            return []
        if msg is None:                  # 이벤트 이름을 먼저 못 봄 → 예전처럼: 링크도 이름 이벤트도 아니면 해석 안 함
            low = data.lower() if isinstance(data, (bytes, bytearray)) else data.encode().lower()
            if b"roblox" not in low and not any(n in data for n in NAME_EVENTS):
                return []
            try:
                msg = s.decode(data)
            except Exception as e:
                self.errors += 1
                self.last_error = f"{type(e).__name__}: {e}"
                return []
        if not isinstance(msg, dict) or msg.get("op") != 0:
            return []
        t, d = msg.get("t"), msg.get("d")
        if not isinstance(d, dict):
            return []
        self._remember(t, d)
        return [(t, d)]

    # ---- 서버·채널 이름 기억
    def _guild(self, g):
        gid = _s(g.get("id"))
        if not gid:
            return
        name = g.get("name") or (g.get("properties") or {}).get("name")
        if name:
            self.guilds[gid] = name
        for ch in (g.get("channels") or []):
            self._channel(ch, gid)
        for th in (g.get("threads") or []):
            self._channel(th, gid)

    def _channel(self, ch, gid=None):
        if not isinstance(ch, dict):
            return
        cid = _s(ch.get("id"))
        if not cid:
            return
        old = self.channels.get(cid, {})
        self.channels[cid] = {
            "name": ch.get("name") or old.get("name") or "",
            "guild": _s(ch.get("guild_id")) or gid or old.get("guild"),
            "parent": _s(ch.get("parent_id")) or old.get("parent"),
            "type": ch.get("type", old.get("type")),
        }

    def _remember(self, t, d):
        if t == "READY":
            self.ready = True
            for g in d.get("guilds") or []:
                if isinstance(g, dict):
                    self._guild(g)
        elif t in ("GUILD_CREATE", "GUILD_UPDATE"):
            self._guild(d)
        elif t in ("CHANNEL_CREATE", "CHANNEL_UPDATE", "THREAD_CREATE", "THREAD_UPDATE"):
            self._channel(d)
        elif t == "THREAD_LIST_SYNC":
            for th in d.get("threads") or []:
                self._channel(th, _s(d.get("guild_id")))

    # ---- 메시지 → 링크 처리용 정보 (hook.js 와 같은 모양)
    def message(self, t, d):
        if t not in ("MESSAGE_CREATE", "MESSAGE_UPDATE"):
            return None
        mid = _s(d.get("id"))
        if not mid:
            return None
        blob = "\n".join(p for p in _collect(d) if isinstance(p, str) and p)
        if "roblox" not in blob.lower():
            return None
        cid = _s(d.get("channel_id"))
        ch = self.channels.get(cid, {})
        gid = _s(d.get("guild_id")) or ch.get("guild")
        author = d.get("author") or {}
        return {
            "id": mid, "channelId": cid, "guildId": gid, "parentId": ch.get("parent"),
            "thread": ch.get("type") in THREAD_TYPES,
            "guildName": self.guilds.get(gid, ""), "channelName": ch.get("name", ""),
            "author": (author.get("global_name") or author.get("username") or "") if isinstance(author, dict) else "",
            "blob": blob, "via": "gateway", "event": t,
        }

    def lookup(self, ids):
        """서버/채널 ID → 이름 (hook.js 의 __lwLookup 과 같은 모양). 모르는 게 있으면 None"""
        out = {}
        for i in ids or []:
            i = str(i)
            if i in self.guilds:
                out[i] = {"type": "guild", "name": self.guilds[i]}
            elif i in self.channels and self.channels[i].get("name"):
                ch = self.channels[i]
                out[i] = {"type": "channel", "name": ch["name"], "guildName": self.guilds.get(ch.get("guild"), ""),
                          "guildId": ch.get("guild")}
            else:
                return None
        return out


def _collect(m, depth=0):
    """메시지에서 링크가 있을 만한 글자 전부 (본문 · 임베드 · 버튼 · 전달된 메시지)"""
    if not isinstance(m, dict) or depth > 3:
        return []
    parts = [m.get("content")]
    for e in m.get("embeds") or []:
        if not isinstance(e, dict):
            continue
        au, ft = e.get("author") or {}, e.get("footer") or {}
        parts += [e.get("url"), e.get("title"), e.get("description"), au.get("name"), au.get("url"), ft.get("text")]
        for f in e.get("fields") or []:
            if isinstance(f, dict):
                parts += [f.get("name"), f.get("value")]

    def walk(c):
        if isinstance(c, dict):
            parts.extend([c.get("url"), c.get("label"), c.get("content")])
            for x in c.get("components") or []:
                walk(x)
    for c in m.get("components") or []:
        walk(c)
    for snap in m.get("message_snapshots") or []:
        if isinstance(snap, dict):
            parts += _collect(snap.get("message"), depth + 1)
    return parts
