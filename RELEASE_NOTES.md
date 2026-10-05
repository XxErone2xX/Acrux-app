## Acrux macro V1.19.2 업데이트
- 미니게임에서 구간이 **바 왼쪽 끝에 붙으면 클릭을 멈춰서** 진행도가 빠지고 실패하던 버그 수정
  - 바 양 끝의 ◇ 표시를 테두리로 보고 지워 버려서 내 위치를 몰랐음 → 이제 끝에 있는 ◇ 도 읽고, 막대가 비어 있으면 맨 왼쪽으로 봄
  - 그래서 구간이 왼쪽 끝에 있어도 계속 눌러서 구간 안에 머무름

## Acrux macro V1.19.2 Update
- Fixed a bug where the macro **stopped clicking once the zone reached the left end of the bar**, so progress drained and the catch failed
  - The ◇ marker at the bar's ends was being erased as a border line, so the position was unknown; the marker at the ends is now read, and an empty fill counts as the far left
  - It now keeps clicking and stays in the zone even when the zone sits at the left end
