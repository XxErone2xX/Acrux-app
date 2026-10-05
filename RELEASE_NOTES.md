## Acrux macro V1.19.3 업데이트
- 미니게임에서 내 위치가 **구간 한가운데인데 클릭해서 구간 오른쪽으로 넘어가던** 문제 수정
  - 바 가운데 남은 시간 숫자의 테두리가 막대 끝을 몇 px 가려서, 막대가 구간 앞에서 끊긴 걸로 보고 "구간 아래"로 잘못 판단했음
  - 이제 막대 끝 바로 뒤에 숫자나 구간 색이 있으면 막대 끝 대신 ◇ 표시를 믿음

## Acrux macro V1.19.3 Update
- Fixed the minigame **clicking while the marker was in the middle of the zone**, pushing it past the right side
  - The outline of the countdown digits hid a few pixels at the end of the fill, so the fill looked like it stopped before the zone and was read as "below the zone"
  - When digits or zone color sit right after the end of the fill, the ◇ marker is now trusted instead of the fill end
