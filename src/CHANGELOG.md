<!-- 업데이트 로그 — Acrux 탭 · 업데이트 로그에 그대로 보임
     새 버전을 올릴 때마다 맨 위에 "# V버전 · 날짜" + RELEASE_NOTES.md 내용을 추가 (릴리스 때 빠졌으면 실패함) -->

# V1.13.1 · 2026-10-04

## Acrux macro V1.13.1 업데이트
- Acrux 탭에 **업데이트 로그** 추가
  - 지금까지 올라온 업데이트에서 바뀐 점을 버전별로 모아서 보여줌 (맨 위가 최신 · 지금 쓰는 버전 표시)
  - 한국어로 보면 한국어, 다른 언어로 보면 영어 내용으로 표시
  - 프로그램 안에 들어 있어서 인터넷 없이도 볼 수 있음

## Acrux macro V1.13.1 Update
- Added an **Update log** to the Acrux tab
  - Shows what changed in each update so far, by version (newest at the top · your current version is marked)
  - Shown in Korean when the app is in Korean, otherwise in English
  - Built into the program, so it works without internet

# V1.13.0 · 2026-10-04

## Acrux macro V1.13.0 업데이트
- 자동 낚시 릴링을 더 안정적으로 수정
  - 전에는 표시가 구간 **가운데**보다 왼쪽이면 눌러서, 너무 자주 눌러 구간 오른쪽으로 넘어가는 일이 많았음
  - 이제 표시가 구간 **왼쪽 끝 근처**까지 떨어졌을 때만 누름
  - 세부 설정에 **목표 위치** 추가 (구간 왼쪽 끝에서 구간 폭의 몇 % · 기본 10)
    - 구간 왼쪽으로 자꾸 빠지면 늘리고, 오른쪽으로 넘어가면 줄이면 됨
- 매크로 기준 위치 설정의 위치 템플릿에서 **화면 비율** 선택 가능
  - 자동(지금 로블록스 창 크기로 계산) · 16:9 · 16:10 · 21:9 · 32:9 · 4:3 · 5:4
  - 16:9 가 아닌 비율은 추정값이라 [상태 확인] 으로 확인하고, 안 맞는 항목은 직접 지정

## Acrux macro V1.13.0 Update
- Made auto fishing reeling more stable
  - Before, it clicked whenever the marker was left of the zone's **center**, so it clicked too often and overshot to the right
  - Now it clicks only when the marker falls near the zone's **left end**
  - Added **Target point** to fine-tuning (% of the zone width from its left end · default 10)
    - Raise it if the marker keeps dropping out to the left, lower it if it overshoots to the right
- The position template in Macro base position settings now lets you pick the **screen ratio**
  - Auto (calculated from the current Roblox window size) · 16:9 · 16:10 · 21:9 · 32:9 · 4:3 · 5:4
  - Ratios other than 16:9 are estimates, so verify with [Check status] and set any that are off yourself

# V1.12.1 · 2026-10-04

## Acrux macro V1.12.1 업데이트
- 자동 낚시 [상태 확인] 을 누르면 로블록스 창을 맨 앞으로 띄운 뒤 확인하도록 수정
  - 전에는 Acrux 창이 로블록스를 가리고 있어서 Acrux 화면을 읽는 바람에 확인이 안 됐음
  - 확인이 끝나면 Acrux 창으로 다시 돌아와서 결과를 보여줌
- [OCR 테스트] 도 읽은 뒤 Acrux 창으로 다시 돌아옴

## Acrux macro V1.12.1 Update
- Auto fishing [Check status] now brings the Roblox window to the front before checking
  - Before, the Acrux window covered Roblox, so it read the Acrux screen and the check failed
  - After checking, it switches back to Acrux to show the result
- [OCR test] also switches back to Acrux after reading

# V1.12.0 · 2026-10-04

## Acrux macro V1.12.0 업데이트
- 매크로 탭 구조 정리
  - **매크로 기능 설정** 맨 위에 **기능 켜기 · 끄기** 탭 추가 — 쓸 기능을 한 곳에서 켜고 끄기 · [전부 켜기] / [전부 끄기]
  - 버튼 위치 · OCR 영역 · 릴링 바 영역 지정은 전부 **매크로 기준 위치 설정** 으로 이동 (기능마다 탭이 따로 있음)
    - 레어 바이옴 자동 팝핑: 버튼 6개 + OCR 영역 · [16:9 적용] · [스나이프 탭 위치 가져오기] · [OCR 테스트]
    - 자동 낚시: Fish 버튼 · 결과창 X · 결과창 제목 · 릴링 바 영역 · [16:9 적용] · [상태 확인]
  - 기능 설정 화면에는 켜기 · 포션 목록 · 세부 설정과 위치가 다 지정됐는지만 표시
- 레어 바이옴 자동 팝핑이 이제 위치를 스나이프 탭 오토 팝핑과 따로 저장함
  - 업데이트 후 처음 한 번은 매크로 기준 위치 설정에서 지정 필요 ([스나이프 탭 위치 가져오기] 로 그대로 복사 가능)
  - 딜레이 · 이름 일치율은 지금처럼 오토 팝핑 설정을 같이 씀

## Acrux macro V1.12.0 Update
- Reorganized the Macro tab
  - Added a **Features on / off** tab at the top of **Macro feature settings** — turn the features you use on and off in one place · [All on] / [All off]
  - All button position, OCR area and reel bar area settings moved to **Macro base position settings** (one tab per feature)
    - Rare biome auto popping: 6 buttons + OCR area · [Apply 16:9] · [Copy from Snipe tab] · [OCR test]
    - Auto fishing: Fish button · result X · result title · reel bar area · [Apply 16:9] · [Check status]
  - The feature screens now only show the on switch, potion lists, fine-tuning, and whether positions are set
- Rare biome auto popping now stores its positions separately from the Snipe tab's auto popping
  - After updating, set them once in Macro base position settings ([Copy from Snipe tab] copies them as-is)
  - Delays and name match still come from the auto popping settings

# V1.11.0 · 2026-10-04

## Acrux macro V1.11.0 업데이트
- 매크로 기능 설정에 **자동 낚시** 추가 (1단계: 제자리 낚시)
  - Fish 클릭 → 입질 대기 → 릴링(구간 안에 표시가 머물도록 자동 클릭) → 결과창 닫기 를 반복
  - 결과(성공 / 쓰레기 / 실패)를 세서 로그에 표시
  - Fish 를 눌러도 반응이 없으면(최대 3번 재확인) 인벤토리가 가득 찬 것으로 보고 정지 — 자동 판매 · 이동은 다음 업데이트에서 추가 예정
  - 위치 템플릿 [16:9 적용] 으로 Fish 버튼 · 낚시 바 · 결과창 위치를 한 번에 채울 수 있고, 직접 지정도 가능
  - [상태 확인] 으로 지금 화면에서 버튼 · 낚시 바를 제대로 읽는지 확인 가능
  - 레어 바이옴 자동 팝핑이 시작되면 낚시를 잠시 멈추고, 끝나면 다시 이어서 진행
  - 스나이프 중에는 동작하지 않음 · F7 로 정지
- 매크로 탭 · 팝핑 설정을 여러 번 바꾸면 처음 바꾼 것만 저장되고 이후 변경이 사라지던 문제 수정

## Acrux macro V1.11.0 Update
- Added **Auto Fishing** to Macro Features (step 1: fishing in place)
  - Repeats: click Fish → wait for a bite → reel (auto-clicks to keep the marker inside the zone) → close the result
  - Counts results (success / junk / fail) and shows them in the log
  - If Fish does nothing (rechecked up to 3 times), the inventory is treated as full and fishing stops — auto selling and movement are coming in a later update
  - The position template [Apply 16:9] fills the Fish button, fishing bar and result positions at once; you can also set them yourself
  - [Check status] shows whether the button and fishing bar are read correctly on the current screen
  - Pauses while rare biome auto popping runs, then resumes
  - Does not run while sniping · F7 to stop
- Fixed only the first change being saved when Macro tab or popping settings were edited several times

# V1.10.5 · 2026-10-04

## Acrux macro V1.10.5 업데이트
- 프로그램을 켰을 때 설정 탭 이름(바이옴 · 스나이프 · 매크로 · Acrux)이 흐릿하게 보이던 문제 수정
  - 작게 그린 글자를 확대하던 방식 → 크게 그린 글자를 줄이는 방식으로 바꿔서 항상 선명함 (위치 · 크기는 그대로)

## Acrux macro V1.10.5 Update
- Fixed the settings tab names (Biome · Snipe · Macro · Acrux) looking blurry when the program opens
  - Text used to be drawn small and scaled up → now drawn large and scaled down, so it stays sharp (same position and size)

# V1.10.4 · 2026-10-04

## Acrux macro V1.10.4 업데이트
- 메인 버튼을 켤 때 색이 칸 밖으로 삐져나와 보이던 애니메이션 수정 (이제 칸 안에서만 아래에서 위로 차오름)
- 튜토리얼이 다른 탭으로 넘어갈 때 다시 미끄러지는 애니메이션으로 넘어감 (강조 박스는 탭이 다 들어온 뒤 제자리를 잡음)

## Acrux macro V1.10.4 Update
- Fixed the color spilling outside the tile while a main button turns on (it now fills up only inside the tile, bottom to top)
- Tutorials switch settings tabs with the slide animation again (the highlight settles once the tab has slid in)

# V1.10.3 · 2026-10-04

## Acrux macro V1.10.3 업데이트
- 튜토리얼 강조 박스가 버튼 옆으로 밀려 보이던 문제 수정
  - 튜토리얼이 다른 탭으로 넘길 때 칸이 미끄러지는 도중에 위치를 재서 생긴 문제 → 탭을 바로 넘기고, 강조할 곳이 움직이면 강조 박스 · 안내창이 따라감
- 튜토리얼 안내 문구를 새 메인 버튼(바이옴 매크로 · 오토 스나이핑의 시작 버튼)에 맞게 수정

## Acrux macro V1.10.3 Update
- Fixed the tutorial highlight box appearing shifted to the side of the button
  - It was measured while the settings tab was still sliding in → the tutorial now switches tabs instantly, and the highlight box · guide follow the target if it moves
- Updated tutorial text for the new main buttons (Start on Biome macro · Auto sniping)

# V1.10.2 · 2026-10-04

## Acrux macro V1.10.2 업데이트
- 메인 화면 '설정' 글자를 조금 내려서 옆 탭 이름과 높이를 맞춤

## Acrux macro V1.10.2 Update
- Moved the 'Settings' label on the main screen down a little to line up with the tab names

# V1.10.1 · 2026-10-04

## Acrux macro V1.10.1 업데이트
- 메인 화면 버튼 순서 변경: 바이옴 매크로 → 매크로 → 오토 스나이핑
- 버튼을 켤 때 색이 자연스럽게 들어오게 함 (칸은 아래에서 위로 은은하게, 버튼은 왼쪽에서 오른쪽으로 차오름)

## Acrux macro V1.10.1 Update
- New main button order: Biome macro → Macro → Auto sniping
- Colors now come in smoothly when a button turns on (the tile glows up from the bottom, the button fills from left to right)

# V1.10.0 · 2026-10-04

## Acrux macro V1.10.0 업데이트
- 메인 화면 시작 버튼을 기능별 버튼 3개로 나눔: 바이옴 매크로 / 오토 스나이핑 / 매크로
  - 각자 따로 켜고 끔 · 켜지면 그 색으로 채워짐 · 버튼마다 지금 상태 표시
  - 버튼 이름을 누르면 아래 설정 탭이 그 기능 탭으로 넘어감
  - 매크로 버튼이 꺼져 있으면 매크로 탭 기능(레어 바이옴 자동 팝핑 등)이 돌지 않음 · 프로그램을 켤 때마다 꺼진 상태로 시작
  - 바이옴 매크로 설정 카드의 켜기 스위치는 메인 버튼으로 옮김
- 설정 탭에 Acrux 탭 추가: OCR 감지 방식(자동 · RapidOCR · 윈도우 OCR) · 언어 · 데이터 폴더 열기
- 오른쪽 위 스나이핑 안정성 설정을 스나이프 탭으로 옮김

## Acrux macro V1.10.0 Update
- Split the main Start button into 3 buttons: Biome macro / Auto sniping / Macro
  - Each turns on and off separately · fills with its color when on · shows its current status
  - Clicking a button's name switches the settings tab below to that feature
  - Macro tab features (rare biome auto popping, etc.) only run while the Macro button is on · starts off every time the program opens
  - Moved the on switch from the Biome macro settings card to the main button
- Added an Acrux tab to settings: OCR detection (Auto · RapidOCR · Windows OCR) · Language · Open data folder
- Moved Snipe stability from the top right into the Snipe tab

# V1.9.0 · 2026-10-04

## Acrux macro V1.9.0 업데이트
- 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버) 추가
  - 지금 켜져 있는 로블록스에서 레어 바이옴(Cyberspace · Glitched · Dreamspace)이 시작되면 바로 포션 사용
  - 스나이핑으로 들어간 서버, 오토 팝핑 · 매크로 복귀가 도는 중에는 작동 안 함
  - 포션 목록 · 켜진 바이옴은 오토 팝핑과 따로, 버튼 위치 · OCR · 딜레이는 오토 팝핑 설정을 같이 씀
  - 시작 전 대기, 다 쓰고 인벤토리 닫기 설정 · 템플릿별 테스트 버튼
  - 바이옴 감지에 바이옴 매크로 설정의 플레이어 이름이 필요함
- 매크로 기능 설정 순서 변경: 레어 바이옴 자동 팝핑 → 자동 낚시 → 상인 자동 구매 → 포션 자동 제작 → 오토 메모리 매치

## Acrux macro V1.9.0 Update
- Added Macro tab · Rare biome auto popping (your server)
  - Uses potions right away when a rare biome (Cyberspace · Glitched · Dreamspace) starts in the Roblox that is open now
  - Does not run in servers joined by sniping, or while auto popping · macro return is running
  - Potion lists · enabled biomes are separate from auto popping; button positions · OCR · delays are shared with auto popping settings
  - Wait before start, close inventory when done · test button per template
  - Biome detection needs the player name in Biome macro settings
- New order in Macro feature settings: Rare biome auto popping → Auto fishing → Merchant auto buy → Potion auto craft → Auto Memory Match

# V1.8.0 · 2026-10-04

## Acrux macro V1.8.0 업데이트
- 매크로 탭에 메뉴 3개 추가: 매크로 기능 설정 / 매크로 기준 위치 설정 / 통계 보기
- 매크로 기능 설정: 레어 바이옴 자동 팝핑(내 서버) · 상인 자동 구매 · 포션 자동 제작 · 메모리 매치 · 자동 낚시 (기능은 준비 중)

## Acrux macro V1.8.0 Update
- Added 3 menus to the Macro tab: Macro feature settings / Macro base position settings / View stats
- Macro feature settings: Rare biome auto popping (your server) · Merchant auto buy · Potion auto craft · Memory Match · Auto fishing (features coming soon)

# V1.7.5 · 2026-10-04

## Acrux macro V1.7.5 업데이트
- 메인 화면 설정 부분을 조금 위로 올림
- 기본 창 크기 변경 (1392 x 903 비율 · 화면이 작으면 비율 그대로 줄임 · 처음 한 번만 적용)

## Acrux macro V1.7.5 Update
- Moved the settings area on the main screen up a little
- New default window size (1392 x 903 ratio · scaled down on smaller screens · applied once)
