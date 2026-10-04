## Acrux macro V1.16.0 업데이트
- 자동 낚시 안정화 강화 (FishSol · Coteab 방식 참고)
  - 미니게임 시작을 두 신호로 확인: 릴링 바 색 + 낚시 창 ◇ 표시가 미니게임 자리로 옮겨갔는지 (두 매크로가 미니게임 시작을 보는 자리와 같음)
    - 색이 잠깐 번쩍이는 화면을 미니게임으로 착각하지 않음 · ◇ 위치가 조금 어긋나 있어도 바가 계속 보이면 미니게임으로 인정
  - 미니게임 중 바가 잠깐 안 보여도 ◇ 가 미니게임 자리에 있으면 끝난 걸로 안 봄 (중간에 클릭을 멈추던 문제 방지)
  - 내 위치를 잠깐 못 찾으면 직전 위치와 속도로 짐작해서 계속 조작 (0.15초까지)
  - 미니게임 중 0.5초마다 로블록스가 맨 앞인지 확인해서 다른 창이 가리면 다시 띄움
  - 결과창 X 연타는 Fish 뿐 아니라 Exit 가 보여도 멈춤
- 실패 대비 (Coteab 방식 참고)
  - 미니게임이 15초 넘게 안 끝나면 멈춘 걸로 보고 결과창 X + UI 내비게이션(\ → S → A → Enter → \)으로 닫기
  - 입질 최대 대기 + 60초 동안 미니게임이 한 번도 안 열리면 화면 복구 · 3번 연속 안 되면 자동 낚시 멈춤
- 매크로 기준 위치 설정의 [상태 확인]에 ◇ 위치(대기 자리 / 미니게임 자리)도 표시

## Acrux macro V1.16.0 Update
- Stronger auto fishing stability (based on how FishSol and Coteab work)
  - Minigame start is confirmed by two signals: the reel bar colors + the fishing window's ◇ moving to its minigame spot (the same spot both macros use to detect the minigame)
    - A brief color flash is no longer mistaken for a minigame · if the ◇ spot is slightly off, it's still accepted when the bar keeps showing
  - If the bar disappears briefly mid-minigame but the ◇ is still at the minigame spot, it no longer treats the minigame as over (prevents clicking stopping midway)
  - If the marker is lost briefly, it keeps steering using the last position and speed (up to 0.15 s)
  - During the minigame it checks every 0.5 s that Roblox is in front, and brings it back if another window covers it
  - The result X spam now stops when Exit shows as well as Fish
- Failsafes (based on Coteab)
  - If a minigame doesn't end within 15 s, it's treated as stuck and closed with the result X + UI navigation (\ → S → A → Enter → \)
  - If no minigame opens for the max bite wait + 60 s, it recovers the screen · after 3 failed recoveries in a row, auto fishing stops
- [Check status] in Macro base position settings now also shows the ◇ position (idle spot / minigame spot)
