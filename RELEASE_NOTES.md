## Acrux macro V3.0.9 업데이트
- 디스코드 봇 연결: 봇이 물어볼 때만 보내게 바꿈
  - 평소에는 데이터 없이 "봇 요청 있어?" 만 확인 [ 작동 중 15초 · 꺼짐 1분마다 상태를 보내던 것 없앰 ]
  - /상태 · /통계 · /로그 명령이 오면 그 명령에 필요한 것만 보냄 · 서버는 봇 답에만 쓰고 저장 안 함
  - 봇 답이 1 ~ 2초 늦게 나옴 [ "생각 중…" 이 먼저 뜸 ]

## Acrux macro V3.0.9 Update
- Discord bot link: the app now sends data only when the bot asks
  - Normally it only checks "any bot request?" with no data [ no more status every 15 s while running · every minute when off ]
  - When /status · /stats · /logs comes, it sends only what that command needs · the server uses it for the reply and doesn't store it
  - Bot replies take 1 – 2 s [ "thinking…" shows first ]
