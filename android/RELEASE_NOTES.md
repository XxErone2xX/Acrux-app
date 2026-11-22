## Acrux Mobile V1.7.2 업데이트
1. 서버 체크에서 게임 메뉴 X와 브섭 관리창 X 위치를 각각 지정합니다. 처음 사용하기 전에 두 위치를 저장해 주세요.

## Acrux Mobile V1.7.2 Update
1. Server checks now use separate saved close-button positions for the game menu and private server panel. Set both before running a check.

## Acrux Mobile V1.7.1 업데이트
1. Private Server Commands 보정 안내를 줄였습니다. 서버 체크는 직접 지정한 버튼 위치만 사용하며, 미지정 시 시작하지 않습니다.

## Acrux Mobile V1.7.1 Update
1. Shortened the Private Server Commands setup instructions. Server checks now require a manually saved button position.

## Acrux Mobile V1.7.0 업데이트
1. 낚시 릴링 분석의 임시 배열 생성을 줄이고 판매 버튼 감지의 중복 화면 분석을 제거했습니다.
2. 글자 인식용 화면을 작은 줄 묶음으로 복사해 임시 메모리를 줄이고, 캡처와 글자 인식 종료 시 자원 정리를 개선했습니다.
3. 닫힌 패널과 기록 탭의 불필요한 갱신을 줄였습니다.

## Acrux Mobile V1.7.0 Update
1. Reduced temporary allocations during reeling and eliminated duplicate pixel scans when detecting shop and dialogue buttons.
2. OCR now copies the captured image in strips to reduce temporary memory. Improved capture and OCR resource cleanup.
3. Reduced unnecessary updates to the closed drawer and log tab.

## Acrux Mobile V1.6.12 업데이트
1. 봇 명령어 → 서버 체크에서 Private Server Commands 버튼 위치를 직접 지정할 수 있습니다. 저장한 위치를 우선 사용하고, 미지정 시 자동으로 찾습니다.

## Acrux Mobile V1.6.12 Update
1. Set the Private Server Commands button position under Bot Commands → Server Check. Server checks use the saved position when available and automatic detection otherwise.

## Acrux Mobile V1.6.11 업데이트
1. 낚시·판매 장소 이동 버튼을 낚시 → 길의 각 장소에 배치했습니다. 이동 → 기준 장소에서는 기준 장소 이동을 테스트할 수 있습니다.

## Acrux Mobile V1.6.11 Update
1. Moved the fishing and selling travel buttons next to their respective routes under Fishing → Routes. The base travel test remains under Movement → Base spot.

## Acrux Mobile V1.6.10 업데이트
1. 튜토리얼과 이동 준비 안내에 카메라 모드를 클래식으로 설정하는 단계를 추가했습니다.

## Acrux Mobile V1.6.10 Update
1. The tutorial and movement setup guide now ask you to set Camera Mode to Classic.

## Acrux Mobile V1.6.9 업데이트
1. 낚시 상점에서 Sell All 글자가 흐리거나 나뉘어 읽혀도 버튼 색과 위치로 판매 보정을 이어갑니다.
2. 판매 확인창의 Sell·Cancel 인식을 보강하고, 보정 도중 오류로 중단되면 기존 위치를 유지합니다.

## Acrux Mobile V1.6.9 Update
1. Fishing Shop calibration now uses button colors and positions when the Sell All label is faint or read as separate words.
2. Improved Sell and Cancel detection in the confirmation dialog. Interrupted calibration preserves previously saved positions.

## Acrux Mobile V1.6.8 업데이트
1. 낚시 앞에 봇 명령어 탭을 추가했습니다. 서버 체크에서 재설정 위치와 게임 메뉴 버튼을 보정할 수 있습니다.
2. 낚시 탭에서 판매 위치와 길을 지워도 이동과 서버 체크 보정은 유지됩니다.

## Acrux Mobile V1.6.8 Update
1. Added Bot commands before Fishing in the app and game panel. Open Server check to calibrate the reset controls and game menu button.
2. Clearing selling positions and routes now preserves movement and server-check controls.

## Acrux Mobile V1.6.7 업데이트
1. 브섭 체크 전에 이동 설정의 위치로 캐릭터를 한 번 재설정합니다.
2. 서버 체크와 복귀 요청의 중복 실행을 막고, 자동 낚시 중에는 확인 후 낚시 장소로 돌아갑니다.

## Acrux Mobile V1.6.7 Update
1. Server checks now reset the character once using the configured movement controls.
2. Server checks and return requests take turns. Automatic fishing returns to its spot after a check.

## Acrux Mobile V1.6.6 업데이트
1. 브섭 체크 시 Private Server Commands를 열고 관리 화면에서 확인합니다.
2. 눈과 아우라 효과가 겹칠 때 낚시 대기 화면의 Fish 글자 인식을 개선했습니다.

## Acrux Mobile V1.6.6 Update
1. Private server checks now open Private Server Commands and verify the management panel.
2. Improved Fish button recognition against snow and aura effects.

## Acrux Mobile V1.6.5 업데이트
1. 자동 판매 중 터치가 누락돼 물고기를 남기고 상점을 닫던 문제를 수정했습니다.

## Acrux Mobile V1.6.5 Update
1. Fixed a missed tap during automatic selling causing the shop to close with fish still in the inventory.

## Acrux Mobile V1.6.4 업데이트
1. 물고기 판매 중 일부만 팔고 상점을 닫던 문제를 수정했습니다. [Sell All에 세 번 연속 반응이 없으면 판매 종료]
2. 일부 기기에서 낚시 결과창의 X 버튼 인식을 개선했습니다.
3. 자동 낚시 중 사용자가 보정한 위치가 바뀌던 문제를 수정했습니다.

## Acrux Mobile V1.6.4 Update
1. Fixed the fishing shop closing after selling only some fish. [Selling stops after three consecutive Sell All attempts without a confirmation.]
2. Improved detection of the fishing result window’s close button on affected devices.
3. Fixed fishing calibration positions shifting during automatic fishing.

## Acrux Mobile V1.6.3 업데이트
- 디버깅 모드를 왼쪽으로 밀어서 켜고 끄게 바꿈 [ 앱 맨 위 · 설정 · 떠 있는 Acrux 창 ]
  - 끝까지 밀었다 놓으면 켜짐 ↔ 꺼짐 [ 톡 누르거나 덜 밀면 안 바뀜 · 실수로 안 바뀌게 ]
  - 켜져 있으면 노란색
- 서버 체크: 폰의 디스코드에서 명령을 보내면 "로블록스가 맨 앞에 없습니다" 로 안 되던 문제 수정
  - 로블록스를 앞으로 가져와서 확인하고, 끝나면 명령을 보낸 앱 [ 디스코드 등 ] 으로 돌아감
  - 로블록스가 앞에 있는데도 "맨 앞에 없음" 으로 나오던 문제 수정 [ 게임 도우미 · 떠 있는 아이콘 같은 작은 창이 뜨면 맨 앞 앱이 바뀐 걸로 봤음 ]

## Acrux Mobile V1.6.3 Update
- Debug mode now turns on and off by sliding to the left [ top of the app · settings · floating Acrux window ]
  - Slide all the way and let go to switch on ↔ off [ a tap or a short slide doesn't change it · prevents accidental toggles ]
  - Yellow while on
- Server check: fixed "Roblox isn't in front" when the command was sent from Discord on the same phone
  - Brings Roblox to the front to check, then goes back to the app you sent it from [ Discord etc. ]
  - Fixed "not in front" even while Roblox was in front [ small windows such as game assistants · floating icons were taken as the front app ]

## Acrux Mobile V1.6.2 업데이트
- 디스코드 봇 서버 체크 추가 [ /서버 체크 · PC 와 같음 · 매크로가 지금 내 브섭에 있는지 확인 ]
  - 게임 메뉴를 한 번 열어 Private Server Commands 버튼이 있으면 내 브섭
  - 자동 낚시 중이면 던지기 전 대기 화면에서 확인 [ 낚시 중간을 끊지 않음 · 끝나면 그대로 이어감 ]
  - 내 브섭이 아니면 봇 답에 [내 서버로 복귀] 버튼 [ 매크로 주인만 · 바이옴 매크로 설정의 브섭 링크로 들어감 · 45초 뒤 자동 낚시 다시 시작 ]
  - 무선 디버깅 (ADB) 이 연결돼 있으면 로블록스를 끈 뒤 들어감 [ 없으면 링크만 엶 ]
- 이동 → 기준 장소에 게임 메뉴 버튼 위치 추가 [ 왼쪽 줄 세 개 버튼 · 서버 체크에 필요 ]

## Acrux Mobile V1.6.2 Update
- Added server check to the Discord bot [ /ps check · same as PC · checks whether your macro is in your private server ]
  - Opens the game menu once; if the Private Server Commands button is there, it's your private server
  - During auto fishing it checks on the idle screen before casting [ doesn't cut a round short · fishing continues afterwards ]
  - If it isn't, the bot reply has a [Return to my server] button [ owner only · joins with the private server link in the biome macro settings · auto fishing restarts after 45 s ]
  - If wireless debugging (ADB) is connected, Roblox is closed first [ otherwise only the link is opened ]
- Added a game menu button position under Movement → Base spot [ the three-line button on the left · needed for server check ]

## Acrux Mobile V1.6.1 업데이트
- 낚시: 낚시 창 자리를 잘못 옮겨서 그 뒤로 낚시가 계속 안 되던 문제 수정 [ 눈 바이옴 등에서 결과창을 못 닫은 뒤 생김 ]
  - 낚시 창을 다시 찾았을 때 크기가 다르면 다른 창으로 보고 안 옮김 · 같은 자리로 두 번 보여야 옮김
  - 옮긴 자리에서 문제가 생기면 [ 미니게임을 못 봄 · 가짜 미니게임 · 결과창을 못 닫음 · Fish 반응 없음 ] 보정한 자리로 바로 되돌림
  - 옮긴 자리가 확인 안 된 채로 멈추면 보정한 자리로 되돌려서 저장 [ 전에는 잘못 옮긴 자리가 저장돼서 다시 켜도 안 됐고 자동 보정을 다시 해야 했음 ]
- 낚시: 입질 뒤 미니게임을 못 보고 지나간 판을 실패로 셈 · 로그에도 남김 [ 전에는 통계에서 빠져서 실패 0 으로 보였음 ]
- 자동 낚시: 잠깐 생긴 오류 한 번으로 멈추던 문제 수정 — 이제 스스로 멈추지 않고 쉬었다가 이어감 [ 정지 버튼 · 화면 캡처가 꺼질 때 · 접근성 권한이 꺼질 때만 멈춤 ]
  - 낚시 중 오류 [ 화면 · 글자 읽기 · 이동 · 판매 중 예외 · 메모리 부족 ]: 쉬었다가 그 자리에서 이어감 · 또 나면 낚시 장소로 다시 감 [ 5초부터 · 이어서 날수록 길게 · 최대 2분 ]
  - 화면 복구로도 안 됨 · 낚시 화면이 안 보임 · 판매 실패 · 연속 실패 30번: 멈추지 않고 낚시 장소로 다시 감 [ 이어서 안 될수록 먼저 쉬는 시간을 늘림 · 바로 → 15초 → 30초 … 최대 5분 · 한 판이라도 끝나면 처음부터 ]
  - 판매하러 갔는데 팔 물고기가 없었으면 다음 두 번은 판매하러 가지 않고 낚시 장소부터 다시 잡음 [ 인벤토리가 아니라 Fish 버튼 자리 · 화면 문제일 수 있어서 · 그 뒤엔 다시 판매하러 감 ]
- 낚시: 가짜 미니게임으로 보고 넘길 때 바를 어떻게 읽었는지 로그에 남김 [ 바 화면 몇 장 중 내 위치 · 구간을 몇 장 읽었는지 ]
- 디버깅 모드 추가 [ 앱 맨 위 UI 버튼 밑 · 설정 · 떠 있는 Acrux 창 첫 화면의 켜기 · 끄기 · 켜져 있으면 글자 노랑 ]
  - 켜면 모든 기능을 돌릴 때 매크로가 보는 칸 · 읽은 것 · 누른 자리를 로블록스 화면에 그림 [ 자동 낚시 · 낚시 보정 · 판매 · 판매 보정 · 이동 · 길 기록 · 조이스틱 · 점프 · 바이옴 매크로 · 실행 중에 켜고 꺼도 바로 바뀜 ]
  - 모든 기능: 위쪽 가운데에 지금 상태 · 누른 자리 [ 흰 꺾쇠 ] · 글자 읽기로 읽은 줄 [ 주황 꺾쇠 ] · 조이스틱 손가락 · 점프 · 끝나면 끝난 이유
  - 낚시: 낚시 테스트처럼 보정된 칸을 그림 · 읽은 칸은 밝게, 못 읽은 칸은 흐리게 [ 칸이 실제 창과 어긋났는지 바로 보임 ]
  - 낚시: 낚시 창 옆에 이번 실행 결과 [ 성공 · 쓰레기 · 실패 · 놓침 ] · 낚시 창에서 읽은 것 [ Fish 버튼 · 미니게임 바 · ◇ ] · 릴링 기록 · 바이옴
  - 릴링: 내 위치 · 구간과 함께 예측 위치 [ 파란 세모 ] · 누름 기준 [ 노란 줄 · 예측이 이보다 왼쪽이면 누름 ] 을 그림
  - 낚시 자동 보정: 이번에 찾은 칸만 그림 [ 전에 보정한 칸은 안 그림 ]
  - 그린 것은 화면 캡처에서 지우고 읽음 [ 매크로가 자기가 그린 칸 · 글자를 화면으로 보지 않게 ]
- 낚시 테스트: 디버깅 모드처럼 읽은 것 · 칸 밝기 · 릴링 기록도 보여 줌 · 누른 자리 표시를 네 귀퉁이 꺾쇠로 바꿈 [ 누른 버튼 · 글자를 안 가림 ]

## Acrux Mobile V1.6.1 Update
- Fishing: fixed fishing failing from then on after the fishing window position was moved to a wrong place [ happened after a result window could not be closed, e.g. in the snowy biome ]
  - When the fishing window is found again with a different size, it is treated as a different window and nothing is moved · it has to be seen at the same spot twice to move
  - If something goes wrong at the moved position [ missed minigame · fake minigame · result window not closed · Fish not responding ], it goes back to the calibrated position right away
  - If it stops before the moved position is confirmed, the calibrated position is put back and saved [ before, the wrong position was saved, so it kept failing after restarting until auto-calibrating again ]
- Fishing: rounds where the minigame was missed after the bite now count as fails and are logged [ before, they were left out of the stats, so it showed 0 fails ]
- Auto fishing: fixed stopping on a single temporary error — it no longer stops by itself, it rests and continues [ it only stops with the Stop button, when screen capture ends or when the accessibility permission is turned off ]
  - Errors while fishing [ screen · text recognition · movement · selling exceptions · out of memory ]: rests and continues where it was · if it happens again, goes back to the fishing spot [ from 5 s · longer each time · up to 2 min ]
  - Screen recovery did not help · fishing screen not visible · selling failed · 30 fails in a row: goes back to the fishing spot instead of stopping [ rests longer first each time it keeps failing · right away → 15 s → 30 s … up to 5 min · resets after any finished round ]
  - If there was nothing to sell last time, the next two times it takes the fishing spot again instead of going to sell [ the problem may be the Fish button position or the screen, not the inventory · after that it goes to sell again ]
- Fishing: when a bar is passed over as a fake minigame, the log shows how the bar was read [ how many of the bar frames had your position and the zone read ]
- Added debug mode [ switch under the UI button at the top of the app · in settings · on the first screen of the floating Acrux window · yellow text while on ]
  - When on, every feature draws what the macro looks at, what it reads and where it taps on the Roblox screen [ auto fishing · fishing calibration · selling · sell calibration · movement · route recording · joystick · jump · biome macro · takes effect right away, even while running ]
  - Every feature: the current state at the top center · tap spots [ white corners ] · lines read by text recognition [ orange corners ] · the joystick finger · jumps · when it ends, why it ended
  - Fishing: draws the calibrated boxes like the fishing test · boxes that are read are bright, boxes that are not are faint [ shows right away if a box is off the real window ]
  - Fishing: next to the fishing window, this run's results [ success · junk · fail · missed ], what is read from the fishing window [ Fish button · minigame bar · ◇ ], reel stats and the biome
  - Reeling: draws the predicted position [ blue triangle ] and the tap line [ yellow line · taps when the prediction is left of it ] along with your position and the zone
  - Fishing calibration: only draws the boxes found this time [ old calibrated boxes are not drawn ]
  - Everything drawn is erased from the screen capture before reading [ so the macro does not see its own boxes and text on screen ]
- Fishing test: also shows what is read, box brightness and reel stats like debug mode · tap spots are now shown as four corner marks [ the tapped button and its text are not covered ]

## Acrux Mobile V1.6.0 업데이트
- 낚시: 릴링 조작을 새로 만듦 [ 기기에 따라 실패율이 95% 넘던 문제 · 보정을 다시 할 필요 없음 ]
  - 원인: 누른 게 화면에 보이기까지 오래 걸리는 기기 [ 터치 전달 + 화면 캡처 0.2초 이상 ] 에서, 누른 게 아직 안 보이는 동안 또 눌러서 구간을 크게 넘어갔다가 떨어지기를 반복함 [ 보정으로는 안 고쳐졌음 ]
  - 이 기기의 지연을 화면을 보며 재서 '지금 누르면 반영될 때의 위치' 로 판단함 [ 판마다 다시 재고 저장 ]
  - 미리 누르기 · 목표 위치 · 터치 지연에 맞춰 기다리기 설정은 없앰 [ 자동으로 맞춤 ] · 낚시 → 세부 설정 → 릴링 측정값에 잰 값이 보임 [ 초기화 가능 ]
  - 미니게임 시작 전 'Ready!' 동안은 안 누름 [ 이때 누른 건 안 먹혀서 처음 예측이 틀어지고, 누르는 자리가 안 되는 것으로 잘못 봤음 ]
  - 누른 게 미니게임에 한 번도 반영되지 않으면 누르는 자리를 바꿔 봄 [ 잘 된 자리는 기억 ]
  - 로그: 릴링마다 터치 수 · 지연 · 구간 안 비율을 한 줄로 남김 [ 제보할 때 같이 보내면 원인 확인이 쉬움 ]
- 낚시: 같은 동작을 끝없이 반복하는 경우를 막음
  - 바처럼 보이는 다른 색 [ 결과창 물고기 사진 등 ] 을 미니게임으로 봐도 4초 안에 알아채고 넘어감 [ 전에는 '릴링 → 결과창 닫기' 만 계속 반복할 수 있었음 ]
  - 릴링이 6번 이어서 실패하면 측정값을 지우고 다시 잼 · 30번 이어서 실패하면 멈춤
  - 결과창이 없었거나 결과를 못 본 판은 통계에 안 넣음 [ 전에는 성공으로 셌음 ]
  - 입질이 없어 Exit 를 눌렀는데 안 먹히면 5초 뒤에 다시 누름 [ 전에는 1초마다 누름 ]
- 낚시: 쓰레기 결과창의 X 를 못 누르던 문제 수정
  - 원인: 1.5.4 에서 넣은 '크기가 다른 결과창은 창 크기로 X 자리를 계산해서 누름' 이 실제로는 한 번도 실행되지 않았음
  - 찾은 X → 보정한 X → 전에 닫혔던 자리 → 창 크기로 계산한 자리 순서로 누르고, 안 닫히면 다음 자리로 넘어감
  - 보정한 자리가 아닌 곳에서 닫혔으면 그 자리를 기억해서 다음부터 바로 누름
- 낚시: 미니게임 바를 더 정확하게 읽음 [ 바가 비쳐 보여서 배경 · 시간에 따라 색이 바뀜 ]
  - 밝은 곳 [ 눈 · 낮 ] 에서 바의 빈 칸이 회색으로 비칠 때 그 회색을 물고기 구간으로 보던 문제 수정
  - 바 위로 눈 · 하늘 같은 밝은 배경이 비칠 때 그걸 내 위치 [ ◇ ] 로 보던 문제 수정 · ◇ 가 배경에 묻혀 안 보이면 막대 끝과 구간 색으로 내 위치를 읽음
  - 밤에 막대 밑에 깔린 구간 [ 막대와 색이 비슷함 ] 을 막대로 보던 문제 수정
- 낚시: Acrux 창 → [낚시] → [낚시 보정] → '칸을 띄우며 낚시 테스트' 추가 [ 자동 보정이 맞게 됐는지 눈으로 확인 ]
  - 자동 보정된 칸을 로블록스 화면에 그린 채로 한 판만 낚시함 [ 대기 창 · Fish 버튼 · 미니게임 창 · 바 · 결과창 · 결과 제목 · X — 그 단계에 보는 칸만 ]
  - 릴링 중엔 읽은 내 위치 [ 흰 세모 ] · 구간 [ 초록 줄 ] 을 바 아래에, 누른 자리는 흰 네모로 보여 줌
  - 칸이 실제 창과 안 맞으면 어느 칸인지 로그에 남기고 끝냄 [ Fish 버튼 칸 · 미니게임 바 칸 등 ]
  - 그린 칸은 화면 캡처에서 지우고 봐서 인식에는 영향 없음
- 성능: 릴링 중 같은 화면을 반복 분석하지 않고 캡처 이미지를 매번 새로 만들지 않음 [ 메모리 사용 감소 ] · 결과창 X 찾기도 줄인 화면을 한 번만 봄

## Acrux Mobile V1.6.0 Update
- Fishing: rebuilt the reeling control [ fixes the 95%+ failure rate on some devices · no need to recalibrate ]
  - Cause: on devices where a tap takes long to show on screen [ touch delivery + screen capture over 0.2 s ], it tapped again while the earlier tap wasn't visible yet, overshooting the zone and falling back repeatedly [ recalibrating didn't fix it ]
  - It now measures this device's delay from the screen and decides by 'where you will be when a tap lands' [ re-measured every reel and saved ]
  - Removed the Press early · Target position · Wait for the touch delay settings [ adjusted automatically ] · the measured values show in Fishing → Detailed settings → Reeling measurements [ can be reset ]
  - No taps during the 'Ready!' phase before the minigame starts [ those taps don't count, which threw off the first predictions and made the tap spot look broken ]
  - If taps never reach the minigame, it tries another tap spot [ the spot that works is remembered ]
  - Log: one line per reel with the tap count, delay and time in zone [ sending it with a report makes the cause easy to find ]
- Fishing: prevented cases where the same action repeats forever
  - If something that looks like the bar [ e.g. a fish picture in the result window ] is mistaken for the minigame, it notices within 4 s and moves on [ before, it could keep repeating 'reel → close result' ]
  - After 6 failed reels in a row the measurements are cleared and re-measured · after 30 in a row it stops
  - Rounds with no result window or an unseen result are not counted in the stats [ they used to count as success ]
  - If the Exit tap after no bite doesn't register, it retries after 5 s [ it used to tap every second ]
- Fishing: fixed the junk result window X not being tapped
  - Cause: 'for a result window of a different size, tap the X spot computed from the window size' added in 1.5.4 never actually ran
  - Taps the found X → calibrated X → spots that closed it before → the spot computed from the window size, moving on to the next if the window doesn't close
  - If a spot other than the calibrated one closed it, that spot is remembered and tapped right away next time
- Fishing: the minigame bar is read more accurately [ the bar is see-through, so its colours change with the background and time of day ]
  - Fixed the empty part of the bar showing through as grey in bright places [ snow · daytime ] being read as the fish zone
  - Fixed bright background above the bar [ snow · sky ] being read as your position [ ◇ ] · if the ◇ blends into the background, your position is read from the bar end and the zone colours
  - Fixed the part of the zone under the bar at night [ close to the bar colour ] being read as the bar
- Fishing: added Acrux window → [Fishing] → [Fishing calibration] → 'Fishing test with boxes shown' [ check by eye that auto-calibration came out right ]
  - Fishes one round while drawing the auto-calibrated boxes on the Roblox screen [ idle window · Fish button · minigame window · bar · result window · result title · X — only the boxes used at that step ]
  - While reeling, your position [ white triangle ] and the zone [ green line ] it read are shown under the bar, and tap spots as white squares
  - If a box doesn't match the real window, the log says which one and the test ends [ Fish button box · minigame bar box, etc. ]
  - The drawn boxes are removed from the screen capture before reading, so recognition isn't affected
- Performance: while reeling, the same screen is no longer analyzed repeatedly and capture images aren't recreated every time [ less memory use ] · finding the result window X also looks at a reduced screen once

## Acrux Mobile V1.5.8 업데이트
- 바이옴 매크로 → 안전 모드 추가 [ 기본: 켜짐 · PC 와 같은 방식 ]
  - 서버 ID 가 1분 이상 그대로일 때만 바이옴 알림을 보냄 [ 실수로 다른 사람 서버에 잠깐 들어갔다 나오면 핑이 안 감 ]
  - 기다리는 동안 시작한 바이옴은 1분이 지나고 아직 그 바이옴이면 보냄 · 그 전에 끝나거나 서버를 옮기면 안 보냄
  - 기다리는 동안 미니 UI 의 바이옴 매크로 아래에 안내가 뜸 · 누르면 1분 타이머를 건너뜀 [ PC 의 K 키 ]
  - 로그에서 서버 ID 를 못 읽는 기기에서는 감시 시작 · 접속 끊김 뒤 1분 동안만 기다림
- 웹후크의 디스코드 링크를 새 초대 링크로 변경 [ 예전 링크 만료 ]

## Acrux Mobile V1.5.8 Update
- Added Biome macro → Safe mode [ default: on · same as PC ]
  - Biome notifications are sent only after the server ID has stayed the same for 1 minute or more [ briefly joining someone else's server by mistake sends no ping ]
  - A biome that starts while waiting is sent once the minute passes if it's still that biome · not sent if it ends or you switch servers before that
  - While waiting, a notice shows under Biome macro in the mini UI · tap it to skip the 1 minute timer [ the K key on PC ]
  - On devices where the server ID can't be read from the log, it only waits 1 minute after watching starts or after a disconnect
- Updated the Discord link in webhooks to the new invite [ the old one expired ]

## Acrux Mobile V1.5.7 업데이트
- 기준 장소 이동: 아우라 도감이 안 열리면 버튼을 다시 누름 [ 최대 3번 ]
  - 전에는 안 열려도 아무 표시 없이 시간으로 넘어가서, 카메라가 안 맞은 채로 걸어 엉뚱한 곳으로 가는 경우가 있었음
  - 끝까지 열린 걸 확인 못 하면 로그에 빨간 줄로 알림 [ 아우라 도감 버튼 위치 확인 ]
- 로그: 카메라 정렬 결과 · 기준 장소로 걷는 시간 [ W+A · A · A+W · 쉼 · D · VIP ] 을 남김 [ 길 오류를 알릴 때 원인 확인용 ]
- 화면 사진 자동 저장 제거 [ 결과창을 못 닫았을 때 · 판매 버튼을 못 찾았을 때 사진첩 Pictures/Acrux 에 저장하던 것 ]

## Acrux Mobile V1.5.7 Update
- Base spot: if the aura collection doesn't open, the button is pressed again [ up to 3 times ]
  - Before, it silently moved on by timing even if it didn't open, so it sometimes walked with the camera unaligned and went to the wrong place
  - If it never confirms the collection opened, a red log line says so [ check the aura collection button position ]
- Log: records the camera align result and the base walking times [ W+A · A · A+W · pause · D · VIP ] [ for finding the cause when reporting route errors ]
- Removed automatic screenshot saving [ the ones saved to Pictures/Acrux when the result couldn't be closed or a sell button wasn't found ]

## Acrux Mobile V1.5.6 업데이트
- 이름 변경: 도감 → 아우라 도감 [ 공통 위치 · 상태 · 로그 ]
- 낚시: 화면이 어두운 기기에서 릴링 미니게임을 계속 실패하던 문제 수정
  - 원인: 화면이 어두우면 [ 기기 밝기 필터 · 어두운 바이옴 등 ] 파란 막대 · ◇ 가 원래보다 어둡게 보여서, 막대를 잡을 구간으로 · 내 위치를 맨 왼쪽으로 잘못 읽음 → 계속 누르기만 해서 오른쪽 끝으로 밀려남
  - 파란 막대는 밝기 대신 파란 정도로 찾음 · ◇ 는 그 자리에서 가장 밝은 값 기준으로 찾음 [ 밝은 화면에서는 그대로 ]

## Acrux Mobile V1.5.6 Update
- Renamed: Collection → Aura collection [ common positions · status · log ]
- Fishing: fixed the reeling minigame failing every time on devices with a dark screen
  - Cause: on a dark screen [ device dimming filter · dark biomes etc. ] the blue bar and ◇ look darker than usual, so the bar was read as the catch zone and your position as the far left → it kept clicking and pushed you to the right end
  - The blue bar is now found by how blue it is instead of brightness · ◇ is found relative to the brightest value there [ unchanged on bright screens ]

## Acrux Mobile V1.5.5 업데이트
- 낚시: 결과창이 보정한 자리와 다르게 뜨면 결과창을 못 닫던 문제 수정
  - 원인: 결과창이 반투명해서 밝은 땅 [ 눈 등 ] 위에서는 창 모양을 못 찾음 · X 는 보정한 X 자리 근처에서만 찾아서, 결과창이 다른 크기로 뜨면 아무것도 안 누름
  - 보정한 X 자리 근처에 없으면 화면 오른쪽 위에서 X 를 한 번 더 찾음
  - 이때는 X 오른쪽에 결과창 테두리가 아래로 길게 이어질 때만 누름 [ 순위표 아이콘 등을 X 로 잘못 누르지 않게 · 두 번 확인한 뒤에만 ]

## Acrux Mobile V1.5.5 Update
- Fishing: fixed the result window not closing when it appears differently from the calibrated one
  - Cause: the result window is translucent, so its shape isn't found over bright ground [ snow etc. ] · the X was only searched near the calibrated X spot, so nothing was tapped when the window appeared in a different size
  - If there is no X near the calibrated spot, the top right of the screen is searched once more
  - In that case the X is tapped only if the result window border runs down to its right [ so leaderboard icons etc. aren't tapped as X · only after seeing it twice ]

## Acrux Mobile V1.5.4 업데이트
- 낚시: 일부 기종에서 쓰레기가 뜨면 결과창을 못 닫던 문제 수정
  - 원인: X 를 잘못 누르지 않게 하면서, X 모양이 안 보이는 결과창은 보정한 결과창과 같은 자리일 때만 누르게 했음 → 크기가 다른 쓰레기 결과창은 아무것도 안 누름
  - 결과창을 닫을 때는 가운데 결과창 모양이면 크기가 달라도 그 창의 X 자리를 누름 [ 두 번 확인한 뒤에만 ]
  - 결과창 구석의 X 는 모양 검사를 느슨하게 [ 쓰레기 결과창 X ]
  - 결과창을 10초 안에 못 닫으면 그 화면을 사진첩 Pictures/Acrux 에 남김 [ 한 번 켤 때 3장까지 · 기종마다 다른 결과창 확인용 ]

## Acrux Mobile V1.5.4 Update
- Fishing: fixed the result window not closing on junk on some devices
  - Cause: to avoid wrong taps, a result window without a visible X was only tapped if it matched the calibrated one → the differently sized junk window got no tap
  - While closing results, a centered result-shaped window now gets its X tapped even if its size differs [ only after seeing it twice ]
  - Looser X shape check inside the result window corner [ junk window X ]
  - If the result can't be closed within 10 s, that screen is saved to Pictures/Acrux [ up to 3 per run · to check device-specific result windows ]

## Acrux Mobile V1.5.3 업데이트
- 낚시 결과창 닫기: 처음 3.5초는 보정한 X 자리만 누름 [ 전: 2초 ]
- 결과창 X 찾기 강화 [ 다른 것을 X 로 잘못 누르던 문제 ]
  - X 모양 확인을 더 엄격하게 [ 두 획이 가운데에서 만나고 · 변 가운데는 비어 있어야 함 ]
  - 0.12초 뒤 다시 확인해서 같은 자리에 또 보일 때만 누름
  - X 가 안 보이는 창은 보정한 결과창과 같은 자리일 때만 누름
  - 보정한 X 자리가 있으면 그 근처만 찾음
- 판매: 상점에 물고기가 있는데 "팔 물고기가 없음" 으로 멈추던 문제 수정
  - 물고기 칸을 고른 뒤 확인이 안 되면 같은 칸을 또 눌러서 고른 게 풀릴 수 있던 문제 [ 화면이 늦게 바뀌면 한 번 더 기다린 뒤에만 다시 누름 ]
  - 골랐는지를 글자 말고 왼쪽 칸 물고기 그림으로도 확인 [ 글자를 잘 못 읽는 폰 ]
  - 남은 물고기를 글자로 못 찾아도 첫 칸에 물고기가 보이면 다시 팔아 봄

## Acrux Mobile V1.5.3 Update
- Closing the fishing result: only the calibrated X spot is tapped for the first 3.5 s [ was 2 s ]
- Stronger result X detection [ it sometimes tapped other things as X ]
  - Stricter X shape check [ both strokes must meet in the middle · edge centers must be empty ]
  - Checked again 0.12 s later and tapped only if still in the same spot
  - A window without a visible X is tapped only if it matches the calibrated result window
  - With a calibrated X spot, only the area around it is searched
- Selling: fixed stopping with "no fish to sell" while the shop still had fish
  - If the pick couldn't be confirmed, the same slot was tapped again, which could unselect the fish [ now waits once more before tapping again ]
  - The pick is also checked by the fish picture in the left panel, not only by text [ phones where text reading is weak ]
  - If leftover fish can't be read as text, a fish visible in the first slot still triggers another try

## Acrux Mobile V1.5.2 업데이트
- 미니 UI: 이동을 위로 · 낚시를 아래로
- 미니 UI 낚시 안을 다시 나눔: 낚시 보정 · 길 · 판매 [ 이동처럼 들어가서 설정 · 낚시 줄에 "N / 3 준비됨" ]
- 앱: 기록 칸 → 로그 칸
- 앱: 기준 장소 설정을 낚시 칸에서 꺼내 낚시 앞의 새 이동 칸으로 [ 다른 기능도 같이 쓰게 ]
- 설정 안내 문구도 새 위치에 맞춤 [ 예: [낚시] → [길] → '길 기록' ]

## Acrux Mobile V1.5.2 Update
- Mini UI: Move is now on top · Fishing below it
- Mini UI Fishing is split into Fishing calibration · Routes · Selling [ open each like Move · the Fishing row shows "N / 3 ready" ]
- App: the Log tab (was 기록) is now called 로그 in Korean
- App: base spot settings moved out of the Fishing tab into a new Move tab before Fishing [ so other features can share them ]
- Setup guide texts updated for the new layout [ e.g. [Fishing] → [Routes] → 'Record route' ]

## Acrux Mobile V1.5.1 업데이트
- 디스코드 봇 /로그: 최근 기록을 최대 100줄까지 보냄 [ 기본 50줄 · 예전 30줄 ]

## Acrux Mobile V1.5.1 Update
- Discord bot /logs: sends up to 100 recent log lines [ default 50 · was 30 ]

## Acrux Mobile V1.5.0 업데이트
- V1.4.25 ~ V1.4.27 이 빌드에 실패해서 나오지 못했던 것을 모두 담은 버전 [ 빌드 오류 수정 ]
- 미니 UI 제목을 Acrux → Acrux Mobile 로
- 홈 맨 위에 디스코드 연결 칸 [ 안 했으면 [연결하기] 를 누르면 바로 브라우저 · 했으면 이름이 보이고 누르면 설정 → 디스코드 연결 ]
- 디스코드 봇 연결: 봇이 물어볼 때만 보냄
  - 평소에는 데이터 없이 "봇 요청 있어?" 만 확인
  - /상태 · /통계 · /로그 명령이 오면 그 명령에 필요한 것만 보냄 · 서버는 봇 답에만 쓰고 저장 안 함

## Acrux Mobile V1.5.0 Update
- Includes everything from V1.4.25 – V1.4.27, which failed to build and were never released [ build error fixed ]
- The mini UI title now says Acrux Mobile instead of Acrux
- Discord link card at the top of Home [ not linked: press [Link] and the browser opens · linked: shows your name, press it to go to Settings → Discord link ]
- Discord bot link: the app sends data only when the bot asks
  - Normally it only checks "any bot request?" with no data
  - When /status · /stats · /logs comes, it sends only what that command needs · the server uses it for the reply and doesn't store it

## Acrux Mobile V1.4.27 업데이트
- 디스코드 봇 연결: 봇이 물어볼 때만 보내게 바꿈
  - 평소에는 데이터 없이 "봇 요청 있어?" 만 확인 [ 15초 · 1분마다 상태를 보내던 것 없앰 ]
  - /상태 · /통계 · /로그 명령이 오면 그 명령에 필요한 것만 보냄 · 서버는 봇 답에만 쓰고 저장 안 함
  - 봇 답이 1 ~ 2초 늦게 나옴 [ "생각 중…" 이 먼저 뜸 ]

## Acrux Mobile V1.4.27 Update
- Discord bot link: the app now sends data only when the bot asks
  - Normally it only checks "any bot request?" with no data [ no more status every 15 s · every minute ]
  - When /status · /stats · /logs comes, it sends only what that command needs · the server uses it for the reply and doesn't store it
  - Bot replies take 1 – 2 s [ "thinking…" shows first ]

## Acrux Mobile V1.4.26 업데이트
- 디스코드 연결을 홈 맨 위에 크게 [ 디스코드 색 칸 ]
  - 연결 안 했으면 [연결하기] 를 누르면 바로 브라우저가 열림 · 연결했으면 이름이 보이고 누르면 설정 → 디스코드 연결 칸으로

## Acrux Mobile V1.4.26 Update
- Discord link is now big at the top of Home [ a Discord-colored card ]
  - Not linked: press [Link] and the browser opens right away · linked: shows your name, press it to go to Settings → Discord link

## Acrux Mobile V1.4.25 업데이트
- 미니 UI 맨 위 이름을 Acrux → Acrux Mobile 로

## Acrux Mobile V1.4.25 Update
- The mini UI title now says Acrux Mobile instead of Acrux

## Acrux Mobile V1.4.24 업데이트
- 길 기록 중 [도착] 버튼이 잘 보이게 [ 꽉 찬 파란 버튼 + 어두운 테두리 · 더 크게 ]
- 길 기록 [취소] 버튼 추가 [ [도착] 아래 · 걸어온 길을 버리고 멈춤 · 예전에 기록한 길은 그대로 ]

## Acrux Mobile V1.4.24 Update
- The [Arrived] button while recording a route is easier to see [ solid blue with a dark border · larger ]
- Added a [Cancel] button for route recording [ below [Arrived] · discards the walk and stops · the previously recorded route is kept ]

## Acrux Mobile V1.4.23 업데이트
- 디스코드 연결 추가 [ 설정 → 디스코드 연결 ]
  - [디스코드 로그인] → 브라우저에서 디스코드로 로그인 → [연결] 을 누르면 끝
  - 연결하면 디스코드 매크로 봇으로 이 매크로를 볼 수 있음: /상태 [ 작동 여부 · 지금 하는 일 · 바이옴 ] · /통계 · /로그
  - 연결한 동안만 상태를 보냄 [ 작동 중 15초 · 꺼져 있으면 1분마다 · 화면은 안 보냄 ]
  - 연결 끊기: 같은 곳에서 [연결 끊기] · 또는 acruxmacro.com/link

## Acrux Mobile V1.4.23 Update
- Added Discord link [ Settings → Discord link ]
  - [Discord login] → log in with Discord in the browser → press [Link] and you're done
  - Once linked, the Discord macro bot can show this macro: /status [ running · what it's doing · biome ] · /stats · /logs
  - Status is sent only while linked [ every 15 s while running · every minute when off · never your screen ]
  - To unlink: [Unlink] in the same place · or acruxmacro.com/link

## Acrux Mobile V1.4.22 업데이트
- 미니 UI 정리
  - 맨 아래 기록 칸 뺌
  - 판매를 낚시와 합침 [ 한 기능 ] · 길 기록도 낚시 안으로 [ 기록하는 부분만 · 길 테스트는 이동 → 기준 장소 ]
  - 기능 목록: 낚시 [ 보정 · 길 기록 · 판매 ] · 이동 [ 조이스틱 · 기준 장소 ]
- 화면이 꺼졌거나 잠금 화면일 때는 Acrux 창 · 안내 창을 숨김 [ 잠금을 풀면 다시 보임 ]
- 기준 장소 W + A 시간을 VIP 여부에 따라 다르게 [ 기본 = VIP 있음 ]
  - VIP 있음 = 2.5초 · VIP 없음 = 3.5초 [ 지금까지와 같음 ]
  - 이동 → 기준 장소 맨 위에서 켜고 끔 · 앱 설정에서 두 시간 다 바꿀 수 있음
- 설정 안내 · 사용법 글을 새 메뉴에 맞게 고침

## Acrux Mobile V1.4.22 Update
- Mini UI cleanup
  - Removed the log lines at the bottom
  - Selling is merged into Fishing [ one feature ] · route recording moved into Fishing too [ recording only · route tests are in Movement → Base spot ]
  - Feature list: Fishing [ calibration · routes · selling ] · Movement [ joystick · base spot ]
- The Acrux window and guide window are hidden while the screen is off or locked [ they come back after unlocking ]
- Base spot W + A time now depends on VIP [ default = have VIP ]
  - With VIP = 2.5 s · without VIP = 3.5 s [ same as before ]
  - Toggle it at the top of Movement → Base spot · both times can be changed in the app settings
- Updated the setup guide and how-to text for the new menu

## Acrux Mobile V1.4.21 업데이트
- 미니 UI 새로 디자인 [ 기능이 늘어나도 그대로 쓸 수 있게 ]
  - 홈: 위에 Acrux · 상태 [ 도는 중이면 실행 시간 ] → 큰 [매크로 시작] 버튼 → 그 밑에 작게 바이옴 매크로 켜기 · 끄기 → 기능 목록
  - 낚시 통계는 미니 UI 에서 뺌 [ 앱에서만 보임 ]
  - 기능 목록: 낚시 · 이동 · 판매 [ 각 줄에 지금 상태 · 덜 된 설정은 주황 ] → 누르면 그 기능 설정 페이지 · ‹ 로 돌아감
  - 이동 · 판매 안에 조이스틱 · 기준 장소 · 길 · 판매 페이지
  - 맨 아래에 최근 기록 두 줄
  - ⚙ 메뉴 · 분류 버튼은 뺌
- 색 · 모양 변경: 더 어두운 바탕 + 파란 강조색 · 가는 구분선 · 위치는 저장됨 / 없음으로 표시
  - 손잡이 세로선: 회색 = 대기 · 파랑 = 도는 중
- 설정 안내: 다른 페이지에 있으면 그 페이지로 가는 줄이나 ‹ 를 먼저 강조 · 안내 글의 ⚙ 도 새 메뉴에 맞게 바꿈

## Acrux Mobile V1.4.21 Update
- Redesigned the mini UI [ built to keep working as more features are added ]
  - Home: Acrux · status at the top [ run time while running ] → a big [Start macro] button → a small Biome macro on · off below it → feature list
  - Fishing stats removed from the mini UI [ only in the app now ]
  - Feature list: Fishing · Movement · Selling [ each row shows its current state · unfinished setup in orange ] → tap to open that feature's settings page · ‹ goes back
  - Movement · Selling holds the Joystick · Base spot · Routes · Selling pages
  - The last two log lines at the bottom
  - Removed the ⚙ menu and category buttons
- New look: darker background + blue accent · thin dividers · positions show Saved / None
  - Handle line: gray = idle · blue = running
- Setup guide: on another page, it highlights the row that leads there or ‹ first · guide texts no longer mention ⚙

## Acrux Mobile V1.4.20 업데이트
- 오른쪽 패널 애니메이션 수정
  - 열고 닫을 때 창 크기를 바꿔서, 기기가 크기 변화를 따로 그리는 동안 패널이 위에서부터 늘어나 보였음 [ 이제 창 크기는 고정하고 안의 패널만 미끄러짐 ]
  - 열 때는 점점 느려지고 (0.22초) 닫을 때는 점점 빨라짐 (0.18초)
  - 패널이 상태 표시줄 · 내비게이션 바 밑까지 덮지 않음 [ 로블록스 전체 화면에서는 그대로 위아래 끝까지 ]
- 손잡이 모양 변경: 네모 → 사다리꼴 [ 화면 쪽이 넓음 ] + 끝에 세로선 하나 · ‹ › 화살표와 ㄱ 장식은 뺌
  - 세로선 색: 파랑 = 대기 · 초록 = 도는 중 · 설정 안내가 가리키면 손잡이 전체가 파랗게
- 패널의 ㄱ 장식도 뺌

## Acrux Mobile V1.4.20 Update
- Fixed the right panel animation
  - Opening and closing used to resize the window, and the device drew the resize separately, so the panel looked like it grew from the top [ the window size is now fixed and only the panel slides ]
  - Opening slows down at the end (0.22 s) and closing speeds up (0.18 s)
  - The panel no longer covers the status bar or navigation bar [ in Roblox full screen it still reaches top to bottom ]
- New handle shape: square → trapezoid [ wider on the screen side ] + one vertical line at the end · removed the ‹ › arrows and the corner brackets
  - Line color: blue = idle · green = running · the whole handle turns blue when the setup guide points at it
- Removed the corner brackets from the panel too

## Acrux Mobile V1.4.19 업데이트
- 조작 창을 오른쪽에서 나오는 패널로 바꿈
  - 닫혀 있을 때는 화면 오른쪽 가운데에 손잡이만 보임 [ 게임 화면을 거의 안 가림 ]
  - 손잡이를 누르거나 왼쪽으로 밀면 위아래로 꽉 찬 패널이 나옴 · 다시 누르거나 패널을 오른쪽으로 밀면 닫힘
  - ⚙ 메뉴는 패널 안에서 펼쳐짐 [ 남은 높이를 다 써서 한 번에 더 많이 보임 ]
  - 보정 · 매크로 · 테스트를 시작하면 패널이 자동으로 닫힘 [ 화면 캡처에 안 찍힘 ]
  - 손잡이 색: 파랑 = 대기 · 초록 = 도는 중 · 길 기록 중 [도착] 은 손잡이 옆에 보임
  - 설정 안내가 패널을 가리키면, 닫혀 있을 때는 손잡이를 먼저 강조함
- 설정의 '조작 창 위치 되돌리기' 는 뺌 [ 자리가 고정이라 필요 없음 ]

## Acrux Mobile V1.4.19 Update
- The control window is now a panel that slides out from the right
  - When closed, only a handle shows at the middle of the right edge [ it barely covers the game ]
  - Tap the handle or swipe left to open a full-height panel · tap again or swipe the panel right to close it
  - The ⚙ menu opens inside the panel [ it uses the remaining height, so more fits at once ]
  - The panel closes by itself when calibration, the macro or a test starts [ so it's not in the screen capture ]
  - Handle color: blue = idle · green = running · while recording a route, [Arrived] shows next to the handle
  - When the setup guide points into the panel while it's closed, the handle is highlighted first
- Removed 'Reset control window position' from settings [ the position is fixed now ]

## Acrux Mobile V1.4.18 업데이트
- 릴링 미리 누르기 값 조정 [ PC V3.0.4 와 같음 ]
  - 구간 끝에서 미리 누르는 여유: 바 폭의 6% → 4% [ 물고기 · 기기에 맞춰 0 ~ 1.6배로 자동 조절되는 건 그대로 ]
  - 떨어지는 중에 앞을 내다보는 시간: 최대 0.15초 → 0.1초

## Acrux Mobile V1.4.18 Update
- Adjusted the reeling early-tap values [ same as PC V3.0.4 ]
  - Early-tap margin at the zone edge: 6% → 4% of the bar width [ still auto-scaled 0 ~ 1.6x per fish · device ]
  - Look-ahead while falling: up to 0.15 s → 0.1 s

## Acrux Mobile V1.4.17 업데이트
- 판매 템플릿 4개 (2400x1080 · 2560x1600 · 3120x1440 두 개) 로 판매 버튼 찾기를 다시 맞춤
- 잔디 위에서 [Sell Fish] 를 못 찾던 문제 수정
  - 초록 [Open Shop] 테두리가 뒤의 잔디와 한 덩어리로 보여서 선택지 줄을 못 찾았음 [ 템플릿 4개 중 3개가 그랬음 ]
  - 이제 파란 [Sell Fish] 부터 찾음 [ 오른쪽 빨간 Leave · 화면 가운데 ] — 템플릿 4개 모두 찾음
- 잔디 위에서 상점 [Sell All] 을 색으로 못 찾던 문제도 같은 방식으로 수정
- 첫 번째 물고기 칸을 안 눌러서 물고기를 못 팔던 문제 수정
  - 첫 칸 자리를 글자로 찾을 때 상점 밖 글자까지 물고기 이름으로 봤음 [ 오른쪽 알림 'Aura Auto Added' · 상점 뒤로 비치는 'Captain Flarg' · 한 줄로 읽힌 이름들의 가운데 ]
  - 이제 첫 칸은 상점 모양 ([Sell All] 크기 · 'Fishing Shop' 제목) 으로 계산하고, 글자는 상점 안 물고기 칸의 이름만 봄
  - 첫 칸을 누른 뒤 왼쪽 칸에 'Weight' 가 보이는지로 골라졌는지 확인 · 안 골라졌으면 다른 자리로 다시 누르고 맞는 자리를 저장

## Acrux Mobile V1.4.17 Update
- Re-tuned sell button detection with 4 sell templates (2400x1080 · 2560x1600 · two 3120x1440)
- Fixed [Sell Fish] not being found over grass
  - The green [Open Shop] border merged with the grass behind it, so the choice row was not found [ 3 of the 4 templates ]
  - It now starts from the blue [Sell Fish] [ red Leave on the right · centered on screen ] — found in all 4 templates
- Fixed the shop [Sell All] color detection over grass the same way
- Fixed fish not being sold because the first fish slot was not tapped
  - When finding the first slot by text, text outside the shop was taken as fish names [ the 'Aura Auto Added' notice on the right · 'Captain Flarg' showing through the shop · the middle of several names read as one line ]
  - The first slot is now computed from the shop layout ([Sell All] size · 'Fishing Shop' title), and only names inside the fish grid are used
  - After tapping the first slot it checks for 'Weight' in the left panel · if it's not selected it taps another spot and saves the one that works

## Acrux Mobile V1.4.16 업데이트
- [Sell Fish] 를 못 찾던 문제 수정
  - 선택지가 서서히 떠오르는 동안 대화창을 0.35초마다 계속 눌러서, 흐린 선택지를 못 보고 끝났음 [ 이제 누른 뒤 1.2초는 보기만 함 ]
  - 색으로 못 찾으면 가로 띠로 다시 찾음 [ 초록 Open Shop → 파랑 Sell Fish → 빨강 Leave 순서 ]
  - 글자가 Roll 과 섞여 읽혀도 찾음 [ 'Fish' 가 들어간 글자 · Open Shop 과 Leave 사이 가운데 ]
  - 판매 자동 보정에서 못 찾으면 저장된 자리를 그대로 씀 [ 처음 보정이면 안내와 함께 멈춤 ]
- 물고기가 남았는데 팔지 않고 상점을 닫던 문제 수정
  - 상점을 연 직후 첫 칸 탭이 씹히면 Sell All 을 눌러도 확인창이 안 떠서 '다 팔았음' 으로 봤음 [ 이제 다시 누를 때마다 첫 칸도 다시 고름 ]
  - 닫기 전에 물고기 이름이 보이는지 확인 · 보이면 첫 칸을 화면에서 다시 찾아서 다시 팖
  - 'Sell All' 글자를 물고기 이름으로 잘못 읽던 문제도 수정
- 판매 중 버튼을 못 찾으면 그 화면을 사진첩 (Pictures/Acrux) 에 남김 [ 문제를 알릴 때 같이 보내 주세요 ]

## Acrux Mobile V1.4.16 Update
- Fixed [Sell Fish] not being found
  - It kept tapping the dialog every 0.35 s while the choices were fading in, so it never saw them [ it now only watches for 1.2 s after a tap ]
  - If the colors are not found, it scans again by horizontal band [ green Open Shop → blue Sell Fish → red Leave ]
  - It also finds it when the text is read mixed with Roll [ text containing 'Fish' · midway between Open Shop and Leave ]
  - Sell auto-calibration keeps the saved position if it still can't find it [ the first calibration stops with a hint ]
- Fixed the shop closing without selling while fish were left
  - If the first-slot tap right after opening the shop was swallowed, Sell All showed no confirm window and it assumed everything was sold [ every retry now selects the first slot again ]
  - Before closing, it checks for fish names · if any, it finds the first slot on screen and sells again
  - Also fixed the 'Sell All' text being mistaken for a fish name
- When a button can't be found while selling, that screen is saved to the gallery (Pictures/Acrux) [ please send it along when reporting a problem ]

## Acrux Mobile V1.4.15 업데이트
- 튜토리얼에서 언어를 바꿀 수 있게 추가
  - 튜토리얼 첫 화면에 한국어 · English · 日本語 버튼 [ 누르면 앱 · Acrux 창 · 튜토리얼이 바로 그 언어로 바뀜 ]
- 튜토리얼 영어 · 일본어 번역 추가 [ 전에는 제목만 번역되고 설명은 한국어로 나왔음 ]
- 렉 걸리는 기종에서 이동 중 조이스틱이 가끔 안 움직이던 문제 수정
  - 조이스틱을 뗐다가 바로 다시 누르거나 아주 짧게 누르면 로블록스가 그 누름을 놓쳤음 [ 뗀 뒤 1ms 만에 다시 누르거나 30ms 만 누름 ]
  - 이제 뗀 뒤 0.06초 쉬고 누르고, 한 번 누르면 0.06초 이상 누름 [ 쉰 만큼 뒤의 동작도 같이 밀어서 걷는 거리는 기록과 같음 ]

## Acrux Mobile V1.4.15 Update
- Added a language picker to the tutorial
  - The first tutorial screen has 한국어 · English · 日本語 buttons [ the app, the Acrux window and the tutorial switch right away ]
- Added English · Japanese translations for the tutorial [ only the titles were translated before; the descriptions stayed in Korean ]
- Fixed the joystick sometimes not moving during movement on laggy phones
  - Releasing and pressing the joystick again right away, or a very short press, could be missed by Roblox [ it re-pressed 1 ms after release, or pressed for only 30 ms ]
  - It now waits 0.06 s after releasing before pressing, and each press lasts at least 0.06 s [ later actions are shifted by the same amount, so the walked distance matches the recording ]

## Acrux Mobile V1.4.14 업데이트
- 화면 캡처가 끊기면 알려 주도록 추가
  - 다른 화면 녹화 앱을 켜면 기종에 따라 Acrux 화면 캡처가 검은 화면만 받거나 멈춤 [ 전에는 '낚시 창을 못 찾음' 으로만 끝나서 이유를 알 수 없었음 ]
  - 검은 화면이 6초 넘게 이어지거나 새 화면이 8초 동안 안 들어오면 기록 · 알림으로 안내 [ 로블록스 로딩 화면은 제외 ]
- 릴링 성공률 개선 [ PC V3.0.3 과 같음 ]
  - 구간을 벗어나기 조금 전에 미리 누름 [ 전에는 구간 왼쪽 끝까지 떨어진 뒤에 눌러서 매번 구간 밖으로 나갔다 들어왔음 ]
  - 눌렀는데도 구간 밖에서 계속 떨어지면 바로 다시 누름 [ 전에는 0.2 ~ 0.5초 기다려서 세게 당기는 물고기를 놓쳤음 ]
  - 미리 누르는 폭은 물고기 · 기기에 맞춰 자동 조절 [ 오른쪽으로 넘어가면 줄이고, 왼쪽으로 빠지면 늘림 ]

## Acrux Mobile V1.4.14 Update
- Added a warning when screen capture is cut off
  - Starting another screen recorder can make Acrux's capture get only a black screen or freeze on some phones [ it used to just end with "fishing window not found" ]
  - Shows a log line and a popup if the screen stays black for over 6 s or no new frames arrive for 8 s [ Roblox loading screens are ignored ]
- Better reeling success rate [ same as PC V3.0.3 ]
  - Taps slightly before the marker leaves the fish zone [ it used to tap only after falling to the zone's left edge, so it left the zone every time ]
  - Taps again right away if the marker keeps falling outside the zone [ it used to wait 0.2 ~ 0.5 s and lost hard-pulling fish ]
  - The early-tap margin adjusts itself to the fish and the device [ smaller after overshooting right, larger after dropping out left ]

## Acrux Mobile V1.4.13 업데이트
- 자동 낚시는 이동 · 판매 설정을 모두 마쳐야 시작되도록 변경
  - 전에는 설정 없이도 시작돼서 그 자리에서 낚시만 하다가 가득 차면 멈췄음
  - [시작] 을 누르면 남은 설정 (조이스틱 · 기준 장소 · 길 · 판매) 을 차례로 안내
- [시작] 하면 항상 낚시 장소로 이동한 뒤 낚시 · 가득 차면 판매하고 돌아옴
  - 이동 · 판매를 켜고 끄는 스위치 제거

## Acrux Mobile V1.4.13 Update
- Auto fishing now starts only after the movement · selling setup is finished
  - It used to start without it, fish in place and stop when the inventory was full
  - Tapping [Start] walks you through the missing setup (joystick · base spot · routes · selling) step by step
- [Start] always walks to the fishing spot first, then sells and comes back when the inventory is full
  - Removed the movement · selling on/off switches

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

