// Acrux 사용자 수 서버 (Cloudflare Workers + Durable Object)
// Acrux 가 켜져 있는 동안 몇 분마다 POST /ping {"id": 설치마다 만든 무작위 번호} 를 보냄
// → 최근 ACTIVE_MS 안에 신호를 보낸 번호 수 = 지금 사용 중인 사람 수
// GET /count 는 세기만 함 (집계에 참여하지 않는 사람도 숫자는 볼 수 있음)
// 무작위 번호 말고는 아무것도 저장하지 않음 (IP · 이름 · 기기 정보 없음)

const ACTIVE_MS = 6 * 60 * 1000;      // 6분 안에 신호가 있으면 사용 중
const NEXT_SEC = 300;                 // 앱이 다음 신호를 보낼 때까지 (초)
const ID_RE = /^[0-9a-f]{32}$/;

export default {
  async fetch(req, env) {
    const url = new URL(req.url);
    const stub = env.COUNTER.get(env.COUNTER.idFromName("global"));
    if (url.pathname === "/ping" && req.method === "POST") {
      let id = "";
      try {
        const body = await req.json();
        id = String(body.id || "");
      } catch (e) {}
      if (!ID_RE.test(id)) return json({ error: "bad id" }, 400);
      return stub.fetch("https://counter/ping?id=" + id);
    }
    if (url.pathname === "/count" && req.method === "GET") {
      return stub.fetch("https://counter/count");
    }
    return json({ error: "not found" }, 404);
  },
};

export class Counter {
  constructor(state) {
    this.seen = new Map();            // 번호 → 마지막 신호 시각 (메모리에만 · 서버가 다시 켜지면 몇 분 안에 다시 채워짐)
  }

  async fetch(req) {
    const url = new URL(req.url);
    const now = Date.now();
    if (url.pathname === "/ping") this.seen.set(url.searchParams.get("id"), now);
    for (const [id, t] of this.seen) if (now - t > ACTIVE_MS) this.seen.delete(id);
    return json({ online: this.seen.size, next: NEXT_SEC });
  }
}

function json(obj, status = 200) {
  return new Response(JSON.stringify(obj), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store" },
  });
}
