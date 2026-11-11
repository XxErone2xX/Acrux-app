# Acrux 알파 (Tauri)

실험(알파) 버전입니다. 정식 버전은 PC V3.0.0 (`src/`) · 모바일 V1.4.x (`android/`) 이고, 알파는 정식 앱의 배포 · 자동 업데이트와 따로 움직입니다.

- 코드: `src-tauri/` (Rust 엔진 · 기기별 입력 · 캡처 · Android 셸) · `frontend/` (TypeScript 화면)
- 번역: `frontend/main.ts` 가 `android/app/src/main/assets/i18n/` 의 tsv 를 읽음 → 번역은 `android/tools/mobile_i18n.py` 에 행을 추가하고 tsv 를 다시 만들어서 같이 커밋
- 정식 앱 로직 (낚시 · 이동 · 바이옴 · 판매) 을 바꾼 커밋은 제목에 `[port]` → 알파에도 옮긴 뒤 아래 '옮긴 커밋' 에 적기

## 받기

- [Acrux-Alpha.apk](https://github.com/XxErone2xX/Acrux-app/releases/download/alpha/Acrux-Alpha.apk) — Android (arm64) · 패키지 `com.acrux.next` (정식 앱과 따로 깔림)
- [Acrux-Alpha.exe](https://github.com/XxErone2xX/Acrux-app/releases/download/alpha/Acrux-Alpha.exe) — Windows
- `src-tauri/` · `frontend/` 가 바뀐 커밋이 main 에 들어오면 `.github/workflows/alpha.yml` 이 자동으로 다시 빌드 (실패 기록은 `alpha-build-log` 릴리스)
- 서명: 비밀값 `ACRUX_ALPHA_KEYSTORE_B64` · `ACRUX_ALPHA_KEYSTORE_PASS` 를 넣으면 그 키로 · 없으면 CI 가 캐시한 디버그 키 (7일 넘게 빌드가 없으면 키가 바뀌어서 한 번 지우고 다시 깔아야 함)

## 현재 상태

Android와 Windows는 기존 앱별 화면을 따릅니다. Android의 TypeScript 화면은 기존 모바일 탭과 카드를 재현하고, Windows는 기존 HTML·CSS·화면 동작을 가져와 TypeScript를 통해 Tauri에 연결합니다. 두 앱은 `src-tauri/`의 Rust 낚시·이동·바이옴 엔진을 공유합니다. 모바일 1.4.7의 이동·터치·숫자 저장 수정과 PC 3.0.0의 화면 변경을 반영했습니다. PC 엔진은 기존 낚시·이동·바이옴 이식을 유지하며 Python 모드 실행과 복구·게임 입장 자동화는 이식 전입니다.

- `npm ci` 후 `npm run build`로 화면을 빌드합니다.
- Windows에서는 `npm run tauri -- dev`로 실행하거나 `npm run tauri -- build`로 패키징합니다.
- Android에서는 JDK 17, Android SDK 36, NDK와 Rust의 `aarch64-linux-android` 타깃을 준비한 뒤 `npm run tauri -- android build --apk --target aarch64 --debug`를 실행합니다. 테스트 패키지 `com.acrux.next`는 기존 앱과 별도로 설치됩니다.

미리보기에는 낚시와 이동·판매, 바이옴 감시·디스코드 알림 흐름이 들어 있습니다. Android에서는 조작 창에서 조이스틱과 버튼 위치를 직접 지정하고 방향 패드의 실제 누름·이동·뗌 시각으로 길을 기록합니다. 이전 미리보기의 0.5초 단위 기록도 읽을 수 있으며 새 타이밍을 적용하려면 길을 다시 기록해야 합니다. Windows에서는 기존 이동 메뉴에서 지점 위치와 도착 시간을 저장합니다. 두 앱 모두 수동으로 지정한 판매 위치를 사용합니다.

Android의 바이옴 기능은 앱 내부 무선 디버깅 페어링과 로그 수집을 사용합니다. 로그 재연결 시 이어 읽기와 2분 정체 감지, 웹후크별 순차 전송·429 재시도를 포함합니다. 사용자 수는 `https://acruxmacro.com/api/`에서 읽습니다.

판매 자동 보정, 조이스틱 자동 검색, PC의 Python 모드 실행·복구·게임 입장 자동화와 나머지 PC 전용 자동화, 기존 정식 앱 설정 이전, 배포 업데이트는 아직 이식 전입니다. 모드 화면은 실행 미지원 상태를 표시합니다. Roblox 안에서 이동과 판매가 동작하는지는 별도 검증이 필요합니다.

### Apple 빌드 확인

Mac은 기존 데스크톱 화면을 사용합니다. Mac 빌드에는 macOS와 Xcode가 필요합니다. 사용자 요청에 따라 iOS는 앞으로의 빌드·실행 검사 대상에서 제외했습니다.

- macOS Intel·Apple Silicon 통합 앱 빌드에는 `npm run build:macos`를 사용합니다. 두 Apple Darwin Rust 타깃이 필요합니다.
- `.github/workflows/apple.yml`은 macOS 통합 앱을 빌드하고 시작을 확인합니다.
- `.github/workflows/alpha.yml`은 `Crossplatform-test`에서도 Android·Windows 빌드를 확인합니다. 이 브랜치의 결과는 작업 실행의 첨부 파일로 제공하며 릴리스 갱신은 main에서 수행합니다.

검증 코드 커밋은 `741e2ab`입니다. GitHub 실행기에서 Windows 실행 파일과 macOS Intel·Apple Silicon 통합 앱을 빌드하고 실제 시작을 확인했습니다. Android ARM64 APK 빌드와 순수 Java 터치 타임라인 회귀 검사도 통과했습니다. Rust 코어 테스트는 16개 모두 통과했습니다.

- [Android·Windows 검사 및 빌드 파일](https://github.com/XxErone2xX/Acrux-app/actions/runs/42938271657)
- [Mac 검사 및 통합 앱](https://github.com/XxErone2xX/Acrux-app/actions/runs/42938271616)

PC의 Python 모드 실행·복구 자동화와 Mac 게임 화면 캡처·입력 연결은 이식 전입니다. Windows·Mac에서 시작 검사를 통과했으며 Roblox 안의 실제 동작은 검증 전입니다.

### Android 메모리 측정

UI 재현 전 Android 16 (SM-S931N)에서 각 앱을 화면에 띄워 PSS를 세 번 측정한 중앙값입니다. Tauri 빌드는 앱과 전용 WebView 렌더러를 합산했습니다.

| 상태 | 기존 1.1.0 | Tauri 기본 렌더링 | Tauri 소프트웨어 렌더링 |
| --- | ---: | ---: | ---: |
| 화면 대기 | 118.7 MiB | 176.6 MiB | 117.6 MiB |
| 화면 캡처 대기 | 126.6 MiB | 187.5 MiB | 125.0 MiB |

미리보기의 Android 화면에는 소프트웨어 렌더링을 적용했습니다. 새 UI와 Roblox에서 낚시 중인 메모리는 별도 검증이 필요합니다.

## 이번 변경 검사

- `npm run build`, `cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --lib` (16개), Windows 타깃 코드 검사, Linux 데스크톱 빌드·시작 확인을 통과했습니다.
- 브라우저에서 PC 3.0 화면 초기화·이전 설정 로드와 모바일 숫자 연속 입력을 검사했습니다. 이 화면 검사는 Tauri 호출을 대체한 상태로 실행했으며, 마지막 입력까지 순서대로 저장 요청이 전달되는지 확인했습니다.
- Android 타임라인의 기기 없는 회귀 검사는 `javac -d /tmp/acrux-pilot-tests src-tauri/gen/android/app/src/main/java/com/acrux/next/{Pilot,TraceEvent}.java src-tauri/tests/android/PilotReplayTest.java` 다음 `java -cp /tmp/acrux-pilot-tests com.acrux.next.PilotReplayTest`로 실행합니다.
- ADB 연결이 없어 새 APK의 휴대폰 실행과 Roblox 동작은 검증 전입니다.

## 옮긴 커밋

- `02977ec` Mobile V1.4.1 (길 기록 · 재생 · 바이옴 이어 읽기) — 이식 기준
- `0d162e8` Mobile V1.4.7까지 — 화면 안 좌표·조이스틱 반지름 보정, 터치 중단 뒤 재누름·시간 보정, 숫자 입력 즉시 순차 저장
- `9f435ed` Mobile V1.4.6 — 판매 전체 경로 최대 3회 재시도, 낚시 화면이 30초 동안 보이지 않을 때 최대 3회 재이동
- `f454513` PC V3.0.0 — 원본 화면·복구 설정·모드 화면 반영, 이전 메뉴 설정을 매크로 탭으로 정리. Python 모드 실행과 복구 자동화는 미이식

## PC 본체 정리

스나이핑·Prime 전용 화면, 연동 옵션, 서버 체류·바이옴 추정·복귀 후 동작 경로, 전용 스타일·번역과 디스코드 통신 의존성을 제거했습니다. 이전 설정을 읽거나 저장할 때 전용 값과 기존 모드 설정을 정리하며 기존 스나이핑 모드의 자동 로드·설치를 차단합니다.

내 서버의 포션·아이템 사용, 상인 기능에서 사용하는 딜레이와 이름 일치율은 공통 위치 → 인벤토리에 모았습니다. 이전 화면에만 저장된 인벤토리 좌표는 비어 있는 공통 위치로 옮기며 이미 지정된 공통 좌표를 우선합니다. 일반 모드용 동작 실행 도구와 게임 복구·입장 기능은 공통 기능으로 유지합니다.

PC 런타임 의존성 변경에 따라 Python 런타임 번호를 5로 올렸습니다. 회귀 검사는 `npm run test:desktop` (3개)과 `python -m unittest discover -s tests -v` (4개)이며 Windows·Mac 빌드 검사에 포함했습니다.
