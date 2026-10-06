## Acrux macro V1.29.7 업데이트
- 물고기를 다 팔았는지 새 방식으로 감지
  - Sell All 을 눌렀는데 확인창이 안 뜨면 물고기가 없는 것으로 봄
  - 확인 Sell 버튼 자리의 작은 칸 픽셀만 봐서 가볍고, 확인창이 뜨면 바로 누름
  - 배경이 비치는 곳(빨간 포탈 앞 등)에서 다 판 걸 못 알아채던 문제 수정
  - "물고기 정보 영역" 설정은 더 이상 필요 없어서 뺌
- 자동 낚시 중 자꾸 Exit 가 눌리던 문제 수정
  - 상인 채팅 확인이 낚시를 잠깐 멈추게 하면서 던진 낚시를 Exit 로 취소하고 있었음
  - 이제 채팅 확인은 화면만 읽고 낚시 · 마우스는 건드리지 않음
  - "채팅창 위치" 설정은 필요 없어서 뺌 (채팅창을 켜 두면 됨)

## Acrux macro V1.29.7 Update
- New way to detect that all fish are sold
  - If no confirm window appears after Sell All, there are no fish left
  - Only checks a small pixel patch at the confirm Sell button, so it is light, and clicks it as soon as it appears
  - Fixed not noticing that everything was sold in spots where the background shows through (e.g. in front of the red portal)
  - Removed the "Fish info area" setting since it is no longer needed
- Fixed Exit being pressed repeatedly during auto fishing
  - The merchant chat check was briefly pausing fishing, which cancelled the cast with Exit
  - The chat check now only reads the screen and leaves fishing and the mouse alone
  - Removed the "Chat window position" setting since it is not needed (just keep the chat open)
