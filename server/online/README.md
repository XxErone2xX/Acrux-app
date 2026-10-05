# Acrux 사용자 수 서버

Acrux 가 켜져 있는 사람 수를 세는 작은 서버예요. Cloudflare Workers 무료 요금제로 돌아가요.

## 올리는 방법 A — 대시보드에서 (명령어 없음 · 추천)

1. 왼쪽 메뉴 **Storage & databases → D1 SQL database → Create** → 이름 `acrux-online` → Create
2. 왼쪽 메뉴 **Compute → Workers & Pages → Create → Start with Hello World** → 이름 `acrux-online` → Deploy
3. 만든 Worker → **Settings → Bindings → Add → D1 database** → Variable name `DB` · 데이터베이스 `acrux-online` → Add Binding
4. Worker 오른쪽 위 **Edit code** → 코드를 전부 지우고 `worker-dashboard.js` 내용을 붙여넣기 → Deploy
5. 주소(`https://acrux-online.<이름>.workers.dev`) 뒤에 `/count` 를 붙여 브라우저로 열어서 `{"online":0,...}` 이 뜨면 성공

## 올리는 방법 B — 명령어로 (Node.js 필요 · Durable Object 버전 `worker.js`)

1. https://dash.cloudflare.com 에서 계정을 만들고 로그인
2. 이 폴더(`server/online`)에서:
   ```
   npx wrangler login
   npx wrangler deploy
   ```
3. 끝나면 `https://acrux-online.<계정이름>.workers.dev` 같은 주소가 나옴
4. 그 주소를 `src/app.py` 의 `ONLINE_URL` 에 넣고 새 버전을 올리면 앱에 사용자 수가 뜸

## 동작

- `POST /ping {"id": "<32자리 무작위 번호>"}` → `{"online": 사용자 수, "next": 다음 신호까지 초}`
- `GET /count` → `{"online": 사용자 수}` (집계에 참여하지 않는 사람용 · 세기만 함)
- 최근 6분 안에 신호를 보낸 번호 수를 셈 · 무작위 번호 말고는 저장하지 않음

## 무료 한도

Workers · Durable Objects 무료 요금제는 하루 10만 요청까지예요. 5분마다 신호를 보내면 동시에 약 300명까지 넉넉해요.
사람이 더 많아지면 `worker.js` 의 `NEXT_SEC` 를 늘리면 돼요 (앱은 서버가 알려준 간격을 따름).
