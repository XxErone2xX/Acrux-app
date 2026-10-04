## Acrux macro V1.13.0 업데이트
- 자동 낚시 릴링을 더 안정적으로 수정
  - 전에는 표시가 구간 **가운데**보다 왼쪽이면 눌러서, 너무 자주 눌러 구간 오른쪽으로 넘어가는 일이 많았음
  - 이제 표시가 구간 **왼쪽 끝 근처**까지 떨어졌을 때만 누름
  - 세부 설정에 **목표 위치** 추가 (구간 왼쪽 끝에서 구간 폭의 몇 % · 기본 10)
    - 구간 왼쪽으로 자꾸 빠지면 늘리고, 오른쪽으로 넘어가면 줄이면 됨
- 매크로 기준 위치 설정의 위치 템플릿에서 **화면 비율** 선택 가능
  - 자동(지금 로블록스 창 크기로 계산) · 16:9 · 16:10 · 21:9 · 32:9 · 4:3 · 5:4
  - 16:9 가 아닌 비율은 추정값이라 [상태 확인] 으로 확인하고, 안 맞는 항목은 직접 지정

## Acrux macro V1.13.0 Update
- Made auto fishing reeling more stable
  - Before, it clicked whenever the marker was left of the zone's **center**, so it clicked too often and overshot to the right
  - Now it clicks only when the marker falls near the zone's **left end**
  - Added **Target point** to fine-tuning (% of the zone width from its left end · default 10)
    - Raise it if the marker keeps dropping out to the left, lower it if it overshoots to the right
- The position template in Macro base position settings now lets you pick the **screen ratio**
  - Auto (calculated from the current Roblox window size) · 16:9 · 16:10 · 21:9 · 32:9 · 4:3 · 5:4
  - Ratios other than 16:9 are estimates, so verify with [Check status] and set any that are off yourself
