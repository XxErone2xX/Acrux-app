## Acrux Mobile V0.6.1 업데이트
- **모바일에서 낚시 창 · 미니게임 창을 못 찾던 문제 수정**
  - 모바일 화면은 창 꺾쇠(흰색) 바깥에 회색 테두리가 한 겹 더 있고, 그 안에 거의 흰색인 점들이 섞여 있음
  - 그 점들이 꺾쇠 모서리에 붙어서 창 모서리로 안 보였음 → 더 밝은 기준으로 한 번 더 찾게 함
  - 보정 중 "릴링 중"에서 멈춰 있던 경우가 이것 때문

## Acrux Mobile V0.6.1 Update
- **Fixed the fishing window and minigame window not being found on mobile**
  - On mobile the white window corners have an extra gray outline with almost-white specks in it
  - Those specks touched the corners so they no longer looked like window corners → frames are now searched again with a brighter threshold
  - This is why calibration got stuck at "Reeling"

## Acrux Mobile V0.6.0 업데이트
- **다시 Shizuku 방식으로**: 키보드 방식은 로블록스가 키를 안 받아서 뺌 · 키(Esc · R · Enter · W · A · S · D · E · O · /)와 마우스(우클릭 Click to Move · Sell All 호버 후 클릭 · 우클릭 드래그)를 Shizuku 로 PC 와 똑같이 보냄
  - 앱 → 준비 → ③ Shizuku [연결] (Shizuku 설치 → 무선 디버깅으로 시작 → 허용)
  - 자동 낚시만 쓸 거면 Shizuku 없이도 됨 (화면 터치)
- Acrux 창 ⚙ → 키 테스트: W 를 1초 눌러서 Shizuku 키 입력이 되는지 바로 확인
- Acrux 키보드 · 접근성 입력기 제거

## Acrux Mobile V0.6.0 Update
- **Back to Shizuku**: removed the keyboard method because Roblox did not accept its keys · keys (Esc · R · Enter · W · A · S · D · E · O · /) and mouse (right-click Click to Move · Sell All hover then click · right-drag) go through Shizuku exactly like on PC
  - App → Setup → ③ Shizuku [Connect] (install Shizuku → start via wireless debugging → allow)
  - Auto fishing alone still works without Shizuku (screen taps)
- Acrux window ⚙ → Key test: holds W for 1 s to check right away whether Shizuku key input works
- Removed Acrux Keyboard and the accessibility input method

## Acrux Mobile V0.5.0 업데이트
- **이동 · 판매 입력을 키보드 방식 하나로 정리**: 키(Esc · R · Enter · W · A · S · D · E · O · / · Space)를 키보드로 진짜 키 신호로 보냄
  - Acrux 키보드 추가: 설정에서 켜고 지금 키보드로 선택하면 이 키보드가 로블록스에 키를 보냄
  - 안드로이드 13 이상은 접근성 입력기로도 한 번 더 시도함 (키보드를 안 바꿔도 됨)
  - 클릭은 키보드로 못 보내서 화면 탭 (우클릭 Click to Move · Sell All 등은 0.12초 꾹 눌렀다 뗌) · 우클릭 드래그는 화면 끌기
  - Acrux 창 ⚙ → 키 테스트: W 를 1초 눌러서 키보드 방식이 되는지 바로 확인
- Shizuku · 터치 모드(조이스틱 · 메뉴 리셋 · 핀치) 제거

## Acrux Mobile V0.5.0 Update
- **Movement and selling input reduced to the keyboard method only**: keys (Esc · R · Enter · W · A · S · D · E · O · / · Space) are sent as real key signals through a keyboard
  - Added Acrux Keyboard: enable it in settings and pick it as the current keyboard so it sends keys to Roblox
  - On Android 13+ the accessibility input method is tried as well (no keyboard switch needed)
  - Clicks cannot go through a keyboard, so they are screen taps (right-click Click to Move, Sell All and so on are held for 0.12 s) · right-drag becomes a screen drag
  - Acrux window ⚙ → Key test: holds W for 1 s to check right away whether the keyboard method works
- Removed Shizuku and touch mode (joystick, menu reset, pinch)

## Acrux Mobile V0.4.0 업데이트
- **Shizuku 없이도 이동 · 판매 가능 (터치 모드)**: Shizuku 가 연결 안 돼 있으면 PC 키 · 마우스를 로블록스 모바일 조작으로 바꿔서 누름
  - W A S D → 조이스틱을 그 방향으로 밀고 있기 (W+A 같은 대각선도 · 키를 바꿀 때 손가락을 안 뗌)
  - Esc → R → Enter → 메뉴 버튼 → Reset Character → Reset (글자로 찾음)
  - 우클릭 → Tap to Move · E → 상호작용 버튼 · Space → 점프 버튼
  - O + 우클릭 드래그 → 화면 끌어 내려다보기 + 두 손가락 줌 아웃
  - Sell All 같은 버튼은 0.12초 동안 눌렀다 뗌 (짧게 누르면 호버만 되고 안 눌리던 문제)
  - 조이스틱 · 메뉴 · 점프 버튼은 기본 자리가 있음 · 상호작용 버튼은 Acrux 창 ⚙ → 터치 조작에서 지정
  - 앱에서 탭 누르는 시간 · 내려다보기 배율 조절
- **키보드 앱 방식 키 (실험 · 안드로이드 13+)**: 켜면 키를 접근성 입력기로 먼저 보내 봄 · 로블록스가 받아 주면 조이스틱 대신 진짜 키로 움직임
- Shizuku 가 연결돼 있으면 지금처럼 PC 와 똑같은 키보드 · 마우스로 움직임

## Acrux Mobile V0.4.0 Update
- **Movement and selling without Shizuku (touch mode)**: when Shizuku is not connected, PC keys and mouse are turned into Roblox mobile controls
  - W A S D → hold the joystick in that direction (diagonals like W+A too · the finger stays down when keys change)
  - Esc → R → Enter → menu button → Reset Character → Reset (found by text)
  - Right-click → Tap to Move · E → interact button · Space → jump button
  - O + right-drag → drag the screen to look down + two-finger zoom out
  - Buttons like Sell All are held for 0.12 s (short taps only hovered and did not click)
  - Joystick, menu and jump buttons have default spots · set the interact button in the Acrux window ⚙ → Touch controls
  - Tap hold time and look-down scale are adjustable in the app
- **Keyboard-app style keys (experimental · Android 13+)**: when on, keys are first sent through the accessibility input method · if Roblox accepts them, it moves with real keys instead of the joystick
- With Shizuku connected it still moves with the same keyboard and mouse as on PC

## Acrux Mobile V0.3.0 업데이트
- **PC 와 똑같은 입력**: Shizuku 로 진짜 키보드 · 마우스 신호를 보냄 (Esc · R · Enter · W · A · S · D · E · O · / · 우클릭)
  - 앱 → 준비 → ③ Shizuku [연결] (Shizuku 설치 → 무선 디버깅으로 시작 → 허용)
  - 자동 낚시만 쓸 거면 Shizuku 없이도 됨 (그때는 화면 터치)
- **Sell All 이 안 눌리던 문제 수정**: PC 처럼 버튼 위에 마우스를 먼저 올리고 (호버) 0.05초 뒤 클릭
- **이동을 PC 버전 그대로**: 리셋 (Esc → R → Enter) → 카메라 정렬 (/ → 채팅 → 도감 → 도감 Exit) → W → W+A → A (끝에 W) → S+D → O 누른 채 우클릭 드래그로 내려다보기 · 줌 아웃
  - 낚시 장소 · 판매 장소 = Click to Move (우클릭) 지점 + 걸리는 시간 · PC 처럼 지점 추가 → 걸어가며 [도착] 으로 시간 재기 · 시간 다시 재기 · 마지막 지점 지우기
  - 이동 시간 (리셋 후 대기 · W · W+A · A · A 끝 W · S+D · 내려다보기 px · O 줌 · 걷기 여유) 을 앱에서 바꿀 수 있음
  - 조이스틱 · 핀치 · 경로 글 편집은 없앰
- **판매도 PC 버전 그대로**: 판매 장소 → E → 대화 넘기기 → Sell Fish → (첫 칸 → Sell All → 확인) 반복 → 상점 닫기 → 낚시 장소
  - 판매 자동 보정: NPC 앞에서 시작하고 E 는 직접 누르기 (PC 와 같음)
- Acrux 창 ⚙ 메뉴: Shizuku 연결 · 공통 위치 (채팅 버튼 · 도감 버튼 · 도감 Exit) · 낚시 장소 / 판매 장소 지점 · 판매 자동 보정 · 판매 한 번 · 이동 테스트

## Acrux Mobile V0.3.0 Update
- **Same input as PC**: sends real keyboard and mouse signals through Shizuku (Esc · R · Enter · W · A · S · D · E · O · / · right-click)
  - App → Setup → ③ Shizuku [Connect] (install Shizuku → start via wireless debugging → allow)
  - Auto fishing alone still works without Shizuku (screen taps)
- **Fixed Sell All not being clicked**: like on PC, the mouse hovers the button first and clicks 0.05 s later
- **Movement now matches the PC version**: reset (Esc → R → Enter) → camera align (/ → chat → collection → collection Exit) → W → W+A → A (W at the end) → S+D → hold O and right-drag to look down · zoom out
  - Fishing / sell spot = Click to Move (right-click) points + walk time · add points like on PC and time them with [Arrived] · re-measure · remove the last point
  - Movement times (wait after reset · W · W+A · A · W at end of A · S+D · look-down px · O zoom · walk margin) are editable in the app
  - Removed the joystick, pinch and text route editor
- **Selling now matches the PC version**: sell spot → E → skip dialog → Sell Fish → (first slot → Sell All → confirm) repeated → close shop → fishing spot
  - Sell auto calibration: start in front of the NPC and press E yourself (same as PC)
- Acrux window ⚙ menu: Shizuku connect · common positions (chat button · collection button · collection Exit) · fishing / sell spot points · sell auto calibration · sell once · movement tests

## Acrux Mobile V0.2.0 업데이트
- **이동** 추가 (PC 버전처럼 기준 장소 → 장소): 시작하면 낚시 장소로 가서 낚시 · 30초 동안 낚시 화면이 안 보이면 다시 이동 (2번까지)
  - 경로 = 한 줄에 동작 하나: 리셋 (메뉴 → Reset Character → Reset · 글자로 찾음) · 대기 · 카메라정렬 · 이동(조이스틱 W A S D WA … · 쉼표로 이어서) · 화면내리기 · 화면돌리기 · 줌아웃 · 줌인 · 점프 · 터치 · 꾹 · 터치이동(Tap to Move) · 상호작용(E 대신)
  - 기준 장소 기본 경로는 PC 와 같은 순서 (리셋 → 카메라 정렬 → W 1초 → W+A 7초 → A 1초 → S+D → 내려다보기 → 줌 아웃)
  - 앱에서 경로 편집 · 잘못 적은 줄은 바로 알려 줌 · 필요한 위치가 빠지면 알려 줌
- **판매** 추가 (PC 판매와 같은 순서): 인벤토리가 가득 차면 기준 장소 → 판매 장소 → 상호작용 → 대화 넘기기 → Sell Fish → (첫 칸 → Sell All → 확인 Sell) 반복 → 상점 닫기 → 낚시 장소
  - 다 팔았는지는 확인창(초록 Sell)으로 · 두 번 연속 팔 물고기가 없으면 낚시 자리 문제로 보고 멈춤
  - 판매 보정: NPC 앞에서 상호작용 버튼을 누르면 대화창 · Sell Fish · 상점 · 확인창 위치를 글자로 찾아 맞춤 (팔지는 않음)
  - 글자 읽기는 폰 안에서만 (구글 ML Kit · 인터넷 안 씀)
- Acrux 창 ⚙ 메뉴: 위치 지정 (로블록스 화면에서 누른 곳) · 지점 추가 (Tap to Move 로 걸어가며 시간 재기 → 경로에 자동 추가) · 판매 보정 · 판매 해 보기 · 이동 테스트

## Acrux Mobile V0.2.0 Update
- Added **movement** (like the PC version, base spot → place): goes to the fishing spot when starting · moves again if the fishing screen is not seen for 30 s (up to 2 times)
  - Route = one action per line: reset (menu → Reset Character → Reset · found by text) · wait · camera align · move (joystick W A S D WA … · chain with commas) · tilt down · rotate · zoom out · zoom in · jump · tap · hold · tap-to-move · interact (instead of E)
  - The default base route follows the PC order (reset → camera align → W 1 s → W+A 7 s → A 1 s → S+D → look down → zoom out)
  - Edit routes in the app · wrong lines and missing positions are reported right away
- Added **selling** (same order as the PC version): when the inventory is full → base spot → sell spot → interact → skip dialog → Sell Fish → (first slot → Sell All → confirm Sell) repeated → close shop → fishing spot
  - Detects when everything is sold via the confirm window (green Sell) · stops if there is nothing to sell twice in a row
  - Sell calibration: interact with the NPC and it finds the dialog, Sell Fish, shop and confirm positions by text (does not sell)
  - Text reading runs on the phone only (Google ML Kit · no internet)
- Acrux window ⚙ menu: set positions (tap on the Roblox screen) · add points (walk with tap-to-move and time it → added to the route) · sell calibration · try selling · movement tests
