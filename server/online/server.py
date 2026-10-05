# Acrux 사용자 수 서버 — 컨테이너 · VPS 용 (파이썬 3.8+ · 설치할 것 없음)
# 실행: python server.py            (포트는 PORT 환경 변수 또는 첫 번째 인자 · 기본 8080)
# POST /ping {"id": 32자리 무작위 번호} → {"online": 사용자 수, "next": 다음 신호까지 초}
# GET  /count                          → {"online": 사용자 수}
# 무작위 번호와 마지막 신호 시각만 메모리에 둠 (파일 · IP · 이름 저장 안 함 · 다시 켜면 몇 분 안에 다시 채워짐)
import json
import os
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ACTIVE = 6 * 60          # 6분 안에 신호가 있으면 사용 중
NEXT = 300               # 앱이 다음 신호를 보낼 때까지 (초)
MAX_IDS = 200000         # 이상한 요청이 메모리를 채우지 않게
ID_RE = re.compile(r"[0-9a-f]{32}")
seen, lock = {}, threading.Lock()


def count():
    now = time.time()
    with lock:
        for k in [k for k, t in seen.items() if now - t > ACTIVE]:
            del seen[k]
        return len(seen)


class Handler(BaseHTTPRequestHandler):
    server_version = "AcruxOnline/1"

    def log_message(self, *a):
        pass

    def _send(self, obj, status=200):
        body = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path.split("?")[0] == "/count":
            return self._send({"online": count(), "next": NEXT})
        self._send({"error": "not found"}, 404)

    def do_POST(self):
        if self.path.split("?")[0] != "/ping":
            return self._send({"error": "not found"}, 404)
        try:
            n = min(int(self.headers.get("Content-Length") or 0), 1024)
            uid = str(json.loads(self.rfile.read(n) or b"{}").get("id", ""))
        except (ValueError, AttributeError):
            uid = ""
        if not ID_RE.fullmatch(uid):
            return self._send({"error": "bad id"}, 400)
        with lock:
            if uid in seen or len(seen) < MAX_IDS:
                seen[uid] = time.time()
        self._send({"online": count(), "next": NEXT})


if __name__ == "__main__":
    port = int(sys.argv[1] if len(sys.argv) > 1 else os.environ.get("PORT", 8080))
    print(f"Acrux online server on :{port}", flush=True)
    ThreadingHTTPServer(("0.0.0.0", port), Handler).serve_forever()
