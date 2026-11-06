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
