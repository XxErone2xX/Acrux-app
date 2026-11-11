## Acrux Mobile V1.4.12 업데이트
- 보정의 첫 입력부터 Exit·입질 상태를 확인해 진행 중인 낚시를 취소하지 않도록 수정
- 재입력·인벤토리 가득 참 판정 직전에 낚시 상태를 다시 확인
- PC V3.0.2와 던지기 대기·보정 재입력 조건을 맞춤

## Acrux Mobile V1.4.12 Update
- Check for an existing cast or bite before the first calibration tap to avoid cancelling ongoing fishing
- Recheck the fishing state before retrying Fish or reporting a full inventory
- Matched cast waits and calibration retry conditions with PC V3.0.2

## Acrux Mobile V1.4.11 업데이트
- 던지는 동작이 느린 폰에서 낚시가 계속 취소되던 문제 수정
  - Fish 를 누른 뒤 반응을 1.5초 → 3초까지 기다림 [ 전에는 다시 누른 게 Exit 가 돼서 취소됨 ]
  - 낚시 자동 보정이 "Fish 를 눌러도 반응이 없음" 으로 끝나던 문제 · 자동 낚시가 인벤토리가 가득 찼다고 잘못 멈추던 문제
- 낚시 자동 보정: 다시 누르기 전에 이미 던져졌는지 확인

## Acrux Mobile V1.4.11 Update
- Fixed fishing getting cancelled over and over on phones with a slow cast
  - Waits up to 3 s (was 1.5 s) for the cast after tapping Fish [ the retry used to hit Exit and cancel it ]
  - Fixes fishing auto-calibration ending with "no response to Fish" and auto fishing stopping with a false "inventory full"
- Fishing auto-calibration checks whether the line is already cast before tapping again

## Acrux Mobile V1.4.10 업데이트
- 앱 아이콘 변경

## Acrux Mobile V1.4.10 Update
- New app icon

## Acrux Mobile V1.4.9 업데이트
- 튜토리얼을 PC 버전처럼 기능마다 하나로 정리
  - 시작 준비 · 자동 낚시 · 이동 · 판매 [ 전에는 이동 설정 · 판매 설정이 따로였음 ]
  - 이미 해 둔 단계는 빼고 남은 것만 안내
- 쓰지 않는 기능의 튜토리얼이 뜨지 않도록 변경
  - 앱을 켤 때는 권한이 없을 때 '시작 준비' 만 표시
  - 자동 낚시: 보정 없이 [시작] 을 눌렀을 때 · 이동 · 판매: 스위치를 켰을 때만 표시
  - 하나가 끝나도 다음 튜토리얼로 줄줄이 넘어가지 않음
  - 새로 설치하면 이동 · 판매 스위치는 꺼진 상태로 시작 [ 쓰던 사람은 그대로 ]

## Acrux Mobile V1.4.9 Update
- Tutorials are now one per feature, like the PC version
  - Getting ready · Auto fishing · Movement · Selling [ movement and selling setup used to be separate ]
  - Steps you've already done are skipped
- Tutorials for features you don't use no longer appear
  - On launch, only 'Getting ready' shows, and only if permissions are missing
  - Auto fishing: shown when you tap [Start] without calibration · Movement · Selling: shown when you turn the switch on
  - Finishing one tutorial no longer opens the next one
  - New installs start with the Movement · Selling switches off [ existing users keep their settings ]

## Acrux Mobile V1.4.8 업데이트
- 앱 아이콘 변경

## Acrux Mobile V1.4.8 Update
- New app icon

## Acrux Mobile V1.4.7 업데이트
- 숫자 설정이 실제로 적용되지 않던 문제 수정 [ 기준 장소 시간 · 판매 · 낚시 숫자 칸 전부 ]
  - 숫자를 고치는 즉시 적용 · 저장 [ 전에는 다른 칸을 눌러야 적용돼서, 숫자만 고치고 로블록스로 가면 예전 값으로 돌았음 ]
  - 0.3 이 0.30000001192… 로 보이던 문제 수정
- 이동 중 터치가 끊겨도 길이 어긋나지 않도록 수정
  - 전에는 걷는 중에 화면을 건드리면 그 걸음의 남은 시간을 통째로 건너뜀 [ 큰 오차의 원인 ]
  - 이제 바로 다시 누르고, 못 걸은 시간만큼 뒤의 길을 늦춰서 간 거리가 기록과 같음 [ 기록에 안내가 남음 ]
- Acrux 창이 조이스틱 위에 있을 때 누름이 0.04초씩 늦게 시작하던 문제 수정

## Acrux Mobile V1.4.7 Update
- Fixed number settings not actually being applied [ base location times · selling · fishing number fields ]
  - Numbers are now applied and saved as you type [ before, you had to tap another field; changing a number and going straight to Roblox ran with the old value ]
  - Fixed 0.3 showing as 0.30000001192…
- Movement no longer goes off course when a touch is interrupted
  - Before, touching the screen while walking skipped the rest of that step [ a cause of large errors ]
  - Now it presses again right away and delays the rest of the route by the lost time, so the distance matches the recording [ noted in the log ]
- Fixed presses starting 0.04 s late when the Acrux window is over the joystick

## Acrux Mobile V1.4.6 업데이트
- 이동에 한 번 실패하면 매크로가 멈추던 문제 수정
  - 판매: 판매 장소에 못 가서 대화창 · 상점을 못 찾으면 처음(리셋)부터 다시 이동 [ 3번까지 ]
  - 낚시 장소로 다시 가는 중 오류가 나도 멈추지 않고 다음 시도로 넘어감 [ 3번까지 · 전에는 2번 ]
  - 3번 모두 실패하면 그때 멈춤 [ 길 · 위치 확인 안내 ]

## Acrux Mobile V1.4.6 Update
- Fixed the macro stopping after a single failed move
  - Selling: if it can't reach the sell spot (dialog or shop not found), it moves again from the start [ reset · up to 3 times ]
  - An error while moving back to the fishing spot no longer stops the macro; it moves on to the next try [ up to 3 times · was 2 ]
  - It stops only after all 3 tries fail [ with a hint to check the route and positions ]

## Acrux Mobile V1.4.5 업데이트
- 클릭 위치 · 조이스틱을 화면 밖에 둘 수 없도록 변경
  - 위치 지정 · 조이스틱 찾기 · 직접 지정 모두 화면 안으로 맞춤
  - 예전에 화면 밖으로 저장된 위치도 앱을 켤 때 화면 안으로 고침 [ 다른 화면 크기 · 방향에서 저장한 경우 등 ]
  - 조이스틱 판이 화면 밖으로 나가면 반지름을 화면 안까지로 줄임

## Acrux Mobile V1.4.5 Update
- Click positions and the joystick can no longer be placed off screen
  - Position picking, joystick detection and manual joystick setup all stay inside the screen
  - Positions saved off screen before are moved inside when the app opens [ e.g. saved on a different screen size or orientation ]
  - If the joystick pad goes off screen, its radius is reduced to fit

## Acrux Mobile V1.4.4 업데이트
- 'Path bounds must not be negative' 오류로 매크로가 멈추던 문제 수정
  - 조이스틱이 화면 끝에 가까울 때 터치 위치가 화면 밖으로 나가던 문제 [ 같은 방향으로 화면 안까지만 누름 ]

## Acrux Mobile V1.4.4 Update
- Fixed the macro stopping with a 'Path bounds must not be negative' error
  - Touches near the screen edge no longer go off screen when the joystick is close to the edge [ presses in the same direction, inside the screen ]

