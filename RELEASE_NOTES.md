## Acrux macro V2.5.3 업데이트
- 바이옴 웹후크 순서가 가끔 바뀌던 버그 수정
  - 바이옴 끝 · 시작 알림이 동시에 나갈 때 Started 가 Ended 보다 먼저 오던 문제
  - 이제 웹후크마다 보낸 순서대로 전송 [ 디스코드 전송 제한에 걸리면 잠깐 기다렸다 다시 보냄 ]

## Acrux macro V2.5.3 Update
- Fixed biome webhooks sometimes arriving out of order
  - When biome end · start alerts went out together, Started could arrive before Ended
  - Webhooks are now sent in order per webhook [ waits and retries when Discord rate limits ]
