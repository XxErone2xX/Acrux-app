## Acrux macro V1.14.2 업데이트
- 자동 낚시 중 로블록스가 버벅이던 원인 3가지 수정
  - 화면 캡처 방식 변경: 매번 윈도우가 모든 창을 다시 합치게 만드는 방식(CAPTUREBLT) → 화면 픽셀만 바로 복사하는 방식 (자주 찍어도 게임 · 커서가 버벅이지 않음)
  - 자동 낚시가 도는 내내 Acrux 우선순위를 '높음'으로 올리던 것 제거 (로블록스와 CPU 를 다투지 않음)
  - 릴링 중 화면 읽기를 1초에 약 250번 → 최대 125번으로 줄임 (게임 화면은 1초에 60번쯤 바뀜)

## Acrux macro V1.14.2 Update
- Fixed 3 causes of Roblox lag during auto fishing
  - New screen capture method: instead of making Windows recompose every window on each capture (CAPTUREBLT), it now copies the screen pixels directly (frequent captures no longer stutter the game or the cursor)
  - Acrux no longer raises its own priority to High for the whole time auto fishing runs (no longer competes with Roblox for the CPU)
  - Reeling reads the screen at most 125 times a second instead of about 250 (the game only updates about 60 times a second)
