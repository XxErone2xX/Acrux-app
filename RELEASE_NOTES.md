## Acrux macro V1.20.0 업데이트
- **메모리 사용량 크게 줄임** (기능은 그대로)
  - OCR(RapidOCR)을 켤 때 미리 불러두지 않고, 읽을 때만 따로 띄웠다가 한동안 안 쓰면 꺼서 메모리를 돌려줌
  - 레어 바이옴 포션 사용이 시작되면 OCR 을 미리 띄워 둬서 이름 읽기가 늦어지지 않음
  - 런타임이 쓰지도 않는 OCR · 이미지 라이브러리를 모든 프로세스에 미리 올리던 것 제거 → 위치 지정 창도 더 빨리 뜸
  - 디스코드 감지: 쓰지 않는 이벤트(온라인 상태 등)는 해석하지 않고, 서버 목록은 이름 · ID 만 읽음 → 디스코드 연결 때 메모리가 튀지 않음
  - 화면(Edge)의 업데이트 확인 · 동기화 · 여분 프로세스 등 필요 없는 백그라운드 작업 끔
- 이번 업데이트는 런타임이 바뀌어서 처음 켤 때 한 번 더 받음

## Acrux macro V1.20.0 Update
- **Much lower memory use** (features unchanged)
  - OCR (RapidOCR) is no longer preloaded at startup; it runs in a separate process only when reading and is closed after a while unused, returning its memory
  - When rare-biome potion use starts, OCR is warmed up in advance so name reading is not delayed
  - The runtime no longer loads unused OCR and image libraries into every process, so position pickers also open faster
  - Discord detection: unused events (presence etc.) are no longer parsed, and the server list only keeps names and IDs, so connecting to Discord no longer spikes memory
  - Unneeded background work in the UI (Edge), such as update checks, sync and spare processes, is turned off
- This update changes the runtime, so it is downloaded once more on first launch
