## Acrux macro V1.17.4 업데이트
- 자동 낚시: 릴링 바 영역이 몇 px 어긋나 있으면 바 속 남은 시간 숫자를 내 위치로 잘못 읽던 문제 수정 (보내주신 기록에서 확인)
  - 미니게임마다 릴링 바의 회색 테두리 줄을 찾아 바의 실제 세로 위치로 자동 보정
  - ◇ 로 읽은 위치가 막대 끝과 너무 다르면 막대 끝을 믿음

## Acrux macro V1.17.4 Update
- Auto fishing: fixed the countdown number inside the bar being read as the marker when the reel bar area was a few px off (found in your log)
  - Each minigame now finds the bar's gray border lines and snaps to the bar's real vertical position
  - If the ◇ position disagrees too much with the end of the fill, the fill end is trusted
