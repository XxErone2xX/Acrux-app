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

