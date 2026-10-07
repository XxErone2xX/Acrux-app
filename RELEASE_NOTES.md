## Acrux macro V1.30.0 업데이트
- 상인 자동 구매: 이미 다녀온 상인에게 또 가던 문제 수정
  - 채팅창에 남아 있는 예전 도착 메시지는 무시하고, 도착 메시지가 새로 늘었을 때만 감
- 물고기 판매: 확인창이 이미 떠 있는 상태면 판매를 끝내 버려서 계속 반복되던 문제 수정
  - 확인창이 떠 있으면 먼저 Sell 을 누르고 이어감
- 리셋을 연달아 여러 번 하던 문제 수정
  - 방금 리셋하고 움직이지 않았으면 다음 리셋은 건너뜀
- 오토 아이템 사용: 던진 낚시를 Exit 로 취소하지 않음
  - 지금 낚시가 끝날 때까지 기다렸다가 아이템을 쓰고 그대로 이어서 낚시
- 자동 낚시: 낚시 장소에 못 갔을 때 계속 반복되던 문제 수정
  - 낚시 화면이 20초 동안 안 보이면 낚시 장소로 다시 이동 (2번까지)
  - 그래도 안 되거나, 판매하러 갔는데 두 번 연속 팔 물고기가 없으면 자동 낚시를 멈춤 (F3 로 다시 켜면 다시 시도)
- 이동: W+A 를 1초 줄이고, 그 뒤에 A 0.75초 → W 0.25초 를 누름

## Acrux macro V1.30.0 Update
- Merchant auto buy: fixed going back to a merchant that was already visited
  - Ignores old arrival messages left in the chat and only goes when a new arrival message appears
- Fish selling: fixed endless repeats when the confirm window was already open
  - If the confirm window is open, presses Sell first and continues
- Fixed resetting several times in a row
  - If the character just reset and has not moved, the next reset is skipped
- Auto item use: no longer cancels a cast with Exit
  - Waits for the current catch to finish, uses the items, then keeps fishing
- Auto fishing: fixed endless repeats when it could not reach the fishing spot
  - If the fishing screen is not seen for 20 s, moves to the fishing spot again (up to 2 times)
  - If that still fails, or a sell trip finds no fish twice in a row, auto fishing stops (turn it back on with F3 to retry)
- Movement: W+A is 1 s shorter, then presses A 0.75 s → W 0.25 s
