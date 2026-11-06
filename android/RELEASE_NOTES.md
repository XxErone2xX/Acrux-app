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
