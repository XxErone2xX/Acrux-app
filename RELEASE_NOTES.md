## Acrux macro V3.2.4 업데이트
- 바이옴 매크로 → 기본 설정 → 안전 모드 추가 [ 기본: 꺼짐 ]
  - 켜면 서버 ID 가 1분 이상 그대로일 때만 바이옴 알림을 보냄 [ 실수로 다른 사람 서버에 잠깐 들어갔다 나오면 핑이 안 감 ]
  - 서버 ID 는 로블록스 로그의 접속 기록에서 읽음 · 같은 서버에 다시 들어가면 타이머가 이어짐
  - 기다리는 동안 시작한 바이옴은 1분이 지나고 아직 그 바이옴이면 보냄 · 그 전에 끝나거나 서버를 옮기면 안 보냄
  - 게임에서 K 를 누르면 지금 서버의 1분 타이머를 건너뜀
  - 기본 설정에 지금 서버 ID · 남은 시간 · 기다리는 바이옴 표시
- 게임 화면 매크로 UI 정리: 오른쪽 상태 창과 위쪽 '건드리지 마세요' 띠를 없애고, 화면 위 가운데에 현재 행동만 표시
  - 안전 모드로 바이옴 알림을 기다리는 중이면 그 아래에 안내 표시 [ 매크로가 꺼져 있어도 ]
- 예전 모드용 '다른 서버 접속 중이면 알림 끔' 제거 [ 매크로가 다른 서버로 들어갈 때만 켜지던 기능 ]
- 쓰지 않는 화면 사진 저장 코드 제거

## Acrux macro V3.2.4 Update
- Added Biome macro → Basic settings → Safe mode [ default: off ]
  - When on, biome notifications are sent only after the server ID has stayed the same for 1 minute or more [ briefly joining someone else's server by mistake sends no ping ]
  - The server ID is read from the join record in the Roblox log · rejoining the same server keeps the timer
  - A biome that starts while waiting is sent once the minute passes if it's still that biome · not sent if it ends or you switch servers before that
  - Press K in game to skip the 1 minute timer for the current server
  - Basic settings show the current server ID · time left · waiting biome
- In-game macro UI cleanup: removed the status panel on the right and the "don't touch" bar at the top · only the current action is shown at the top center
  - While safe mode is holding a biome notification, a notice is shown below it [ even when the macro is off ]
- Removed the old mod-only "no notifications while in another server" [ it only turned on when the macro itself joined another server ]
- Removed unused screenshot saving code
