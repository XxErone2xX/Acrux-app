# Acrux 알파 (Tauri)

실험(알파) 버전입니다. 정식 버전은 PC V3.0.0 (`src/`) · 모바일 V1.4.x (`android/`) 이고, 알파는 정식 앱의 배포 · 자동 업데이트와 따로 움직입니다.

- 코드: `src-tauri/` (Rust 엔진 · 기기별 입력 · 캡처 · Android 셸) · `frontend/` (TypeScript 화면)
- 번역: `frontend/main.ts` 가 `android/app/src/main/assets/i18n/` 의 tsv 를 읽음 → 번역은 `android/tools/mobile_i18n.py` 에 행을 추가하고 tsv 를 다시 만들어서 같이 커밋
- 정식 앱 로직 (낚시 · 이동 · 바이옴 · 판매) 을 바꾼 커밋은 제목에 `[port]` → 알파에도 옮긴 뒤 아래 '옮긴 커밋' 에 적기

## 개발 브랜치와 기기 테스트

- `dev`는 크로스플랫폼 개발용 브랜치입니다. 기존 `Crossplatform-test`를 이름 변경했습니다.
- `main`은 정식 PC Python 앱과 모바일 Java 앱의 패치·배포 기준입니다. 정식 앱의 수정은 `dev`에 이식하고 아래에 이식 기록을 남깁니다.
- DUA(Device Use Agent)를 매크로의 주 실행 경로로 개발합니다. Windows와 Linux(X11/XWayland Sober)에서 Headless와 Screenshot 루트를 연결했습니다. Phone 적용은 후속 작업입니다.
- 테스트는 크로스플랫폼 빌드·회귀 검사와 실제 기기에서의 시작·설정·보정·매크로 실행 확인을 함께 사용합니다. 기기·OS·앱 버전·결과를 기록하고, 기기에 접근할 수 없는 항목은 미검증으로 기록합니다.
- Linux·Windows·Android·macOS가 빌드·검사 대상입니다. iOS는 빌드·실행 검사에서 제외합니다.
- `dev`의 Android·Windows·Linux·macOS 빌드 결과는 GitHub Actions 실행의 첨부 파일로 받습니다.

## 받기

- [Acrux-Alpha.apk](https://github.com/XxErone2xX/Acrux-app/releases/download/alpha/Acrux-Alpha.apk) — Android (arm64) · 패키지 `com.acrux.next` (정식 앱과 따로 깔림)
- [Acrux-Alpha.exe](https://github.com/XxErone2xX/Acrux-app/releases/download/alpha/Acrux-Alpha.exe) — Windows
- `src-tauri/` · `frontend/` 가 바뀐 커밋이 main 에 들어오면 `.github/workflows/alpha.yml` 이 자동으로 다시 빌드 (실패 기록은 `alpha-build-log` 릴리스)
- 서명: 비밀값 `ACRUX_ALPHA_KEYSTORE_B64` · `ACRUX_ALPHA_KEYSTORE_PASS` 를 넣으면 그 키로 · 없으면 CI 가 캐시한 디버그 키 (7일 넘게 빌드가 없으면 키가 바뀌어서 한 번 지우고 다시 깔아야 함)

## 현재 상태

Android와 Windows는 기존 앱별 화면을 따릅니다. Android의 TypeScript 화면은 기존 모바일 탭과 카드를 재현하고, Windows는 기존 HTML·CSS·화면 동작을 가져와 TypeScript를 통해 Tauri에 연결합니다. 두 앱은 `src-tauri/`의 Rust 낚시·이동·바이옴 엔진을 공유합니다. 모바일 1.4.7의 이동·터치·숫자 저장 수정과 PC 3.0.0의 화면 변경을 반영했습니다. PC 엔진은 기존 낚시·이동·바이옴 이식을 유지하며 Python 모드 실행과 복구·게임 입장 자동화는 이식 전입니다.

- `npm ci` 후 `npm run build`로 화면을 빌드합니다.
- Windows에서는 `npm run tauri -- dev`로 실행하거나 `npm run tauri -- build`로 패키징합니다.
- Linux에서는 `npm ci` 후 `npm run build:linux -- --ci`로 빌드합니다. 실행 파일은 `src-tauri/target/release/acrux`이며 Sober 연결 조건은 아래를 참고하세요.
- Android에서는 JDK 17, Android SDK 36, NDK와 Rust의 `aarch64-linux-android` 타깃을 준비한 뒤 `npm run tauri -- android build --apk --target aarch64 --debug`를 실행합니다. 테스트 패키지 `com.acrux.next`는 기존 앱과 별도로 설치됩니다.

미리보기에는 낚시와 이동·판매, 바이옴 감시·디스코드 알림 흐름이 들어 있습니다. Android에서는 조작 창에서 조이스틱과 버튼 위치를 직접 지정하고 방향 패드의 실제 누름·이동·뗌 시각으로 길을 기록합니다. 이전 미리보기의 0.5초 단위 기록도 읽을 수 있으며 새 타이밍을 적용하려면 길을 다시 기록해야 합니다. Windows에서는 기존 이동 메뉴에서 지점 위치와 도착 시간을 저장합니다. 두 앱 모두 수동으로 지정한 판매 위치를 사용합니다.

Android의 바이옴 기능은 앱 내부 무선 디버깅 페어링과 로그 수집을 사용합니다. 로그 재연결 시 이어 읽기와 2분 정체 감지, 웹후크별 순차 전송·429 재시도를 포함합니다. 사용자 수는 `https://acruxmacro.com/api/`에서 읽습니다.

판매 자동 보정, 조이스틱 자동 검색, PC의 Python 모드 실행·복구·게임 입장 자동화와 나머지 PC 전용 자동화, 기존 정식 앱 설정 이전, 배포 업데이트는 아직 이식 전입니다. 모드 화면은 실행 미지원 상태를 표시합니다. Roblox 안에서 이동과 판매가 동작하는지는 별도 검증이 필요합니다.

### 데스크톱 DUA

기존 메인 화면의 매크로 시작 버튼이 TypeScript 연결부의 `desktop_start`를 통해 Rust DUA를 시작합니다. DUA는 기존 낚시·이동·판매 엔진을 사용하면서 작업 상태에 따라 다음 실행 경로를 고릅니다. 판단은 코드에 정의한 규칙으로 수행하며 AI 모델·추론 API를 추가하지 않았습니다. 이번 구현은 `dev`에 적용하며 정식 `main`의 Python·Java 앱은 변경하지 않습니다.

- **Headless**는 내부 작업 상태와 구조화된 이동·키 입력·대기를 처리합니다. 이 경로에서는 게임 화면 캡처를 획득하거나 읽지 않습니다. 데스크톱 입력은 대상 게임 창이 앞에 있을 때 수행합니다.
- **Screenshot**은 자동 보정, 낚시의 던지기·입질·릴링·결과, 판매 화면의 확인 단계에서 캡처를 요청합니다. 구조화된 이동·대기 단계로 넘어가면 Headless로 돌아갑니다. 판매도 이동·대기 전체를 계속 캡처하지 않고 화면 확인 단계에서만 캡처를 사용합니다.

보정값이 없으면 낚시를 시작하기 전에 자동 보정하고 이어서 낚시합니다. 낚시 장소 이동을 설정했으면 해당 이동을 마친 뒤 보정합니다. 현재 실행할 수 있는 기능은 자동 낚시와 이에 연결된 이동·판매입니다. 다른 기능이 함께 켜져 있거나 실행할 기능이 없으면 시작 요청을 오류로 반환합니다. 설정 화면의 기존 세부 기능들은 이식 상태에 따라 별도로 구현해야 합니다.

공통 위치 화면의 DUA 패널에는 현재 경로·판단 이유·Headless 처리 횟수·실제 스크린샷 처리 횟수·경로 전환 횟수를 표시합니다. 메인 매크로 타일도 DUA 상태를 우선 표시합니다. 실행 중 활성 기능 목록을 바꾸면 중지하고 다시 시작하도록 안내하며, 일반 설정 편집으로 실행 중인 DUA를 다시 시작하지 않습니다.

메인 정지 버튼과 F3·F7로 실행을 중지합니다. 중지하거나 작업을 전환하면 실행 세대를 바꾸어 이전 작업에서 계산한 입력이 뒤늦게 실행되지 않게 합니다. 캡처·입력 직전에는 대상 창과 크기를 다시 확인하며 이동 중 창 크기가 바뀌면 해당 경로를 중지합니다.

게임 캡처·입력 연결은 Windows와 Linux의 X11/XWayland Sober에 있습니다. macOS 게임 DUA용 캡처·입력 연결은 구현 전입니다. Android는 기존 캡처 프레임을 Rust 엔진에 전달하는 방식을 유지하고 DUA 적용은 후속 작업으로 남깁니다. iOS는 빌드·실행 검사에서 제외합니다.

Windows DUA 도입 때 `npm run build`, `npm run test:desktop` 11개, Rust 코어 검사 27개 (Windows는 시작 요청 세대 검사까지 28개), Python 회귀 검사 5개와 Windows 타깃 코드 검사를 통과했습니다. 검사는 두 루트의 전환, 캡처 없는 이동·대기, 판매 확인의 캡처 요구와 판매 후 낚시 복귀, 보정 후 낚시 재개, 중지·창 크기 변경·이전 입력 취소와 화면 연결부의 시작·오류 처리를 포함합니다. Linux 앱을 별도 임시 설정으로 빌드하고 5초 시작·설정 초기화를 확인했습니다. 브라우저에서는 시험용 상태 응답으로 두 경로 패널의 표시를 확인했습니다. 실제 Roblox에서 낚시·이동·판매 동작은 미검증입니다. 아래 Apple 실행 기록은 DUA 도입 전 커밋의 결과입니다.

### Linux · Sober 실행

첫 Linux 어댑터는 Rust의 X11 프로토콜 연결과 XTEST로 Sober의 X11/XWayland 창을 캡처·조작합니다. Rust/Tauri의 Linux 빌드 의존성과 XTEST 확장을 지원하는 X11 서버가 필요합니다. 이번 연결에도 AI 모델·추론 API는 사용하지 않습니다. 네이티브 Wayland로 실행된 Sober는 이 어댑터의 지원 범위에서 제외합니다.

```sh
npm ci
npm run build:linux -- --ci
./src-tauri/target/release/acrux
```

현재 실행 중인 Sober는 사용자가 먼저 직접 종료해야 합니다. 다음 명령은 새로 시작하는 Sober에만 X11을 적용하며 지속적인 Flatpak override 설정을 저장하지 않습니다. 앱을 강제로 종료하는 명령도 실행하지 않습니다.

```sh
flatpak run --socket=x11 --nosocket=wayland --env=SDL_VIDEO_DRIVER=x11 org.vinegarhq.Sober
```

`--socket`·`--nosocket`은 실행 시 창 서버 연결을 지정하는 [Flatpak run 옵션](https://docs.flatpak.org/en/latest/flatpak-command-reference.html#flatpak-run)입니다. `SDL_VIDEO_DRIVER=x11`은 [SDL3의 비디오 드라이버 선택 변수](https://wiki.libsdl.org/SDL3/SDL_HINT_VIDEO_DRIVER)이며 SDL2의 `SDL_VIDEODRIVER`와 이름이 다릅니다.

Headless 루트는 캡처 없이 이동·키 입력·대기를 처리합니다. Screenshot 루트가 관찰을 요청할 때 Sober 클라이언트 영역을 캡처해 공유 Rust 엔진에 전달합니다. 입력 전에 전면 창·창 크기·실행 세대를 확인하고, F3·F7이나 메인 정지 버튼으로 취소합니다.

Linux 최적화 빌드와 격리된 X11에서 앱의 5초 시작·설정 초기화를 확인했습니다. Rust 기본 검사 31개, 별도로 실행한 실제 X11 캡처·입력 검사 1개, 화면 연결부 11개와 Python 회귀 5개를 통과했습니다. 실제 X11 검사에는 창 관리자 프레임 안의 클라이언트 캡처에서 보정·Fish 클릭, 상대 이동, 정지·전면 변경·크기 변경 시 키 해제, F7과 이전 입력 거절, 창 종료 후 캡처 오류가 포함됩니다. Windows 타깃 코드 검사도 통과했습니다.

격리 검사는 `xvfb-run -a env ACRUX_X11_TEST=1 cargo test --locked --manifest-path src-tauri/Cargo.toml --lib linux::tests::native_x11_capture_and_input -- --ignored --test-threads=1`로 실행합니다. 실제 사용자 화면에서 잘못 실행하지 않도록 전용 `DISPLAY=:90` 이상과 명시적인 시험 환경 변수가 필요합니다. Linux 빌드·검사·시작 확인을 `dev`의 Alpha 작업에 추가했습니다.

현재 Codex 실행 환경에서는 실행 중인 Sober의 X11 연결에 접근하지 못해 실제 게임 검사를 수행하지 못했습니다. Sober에서 보정·낚시·이동·판매가 동작하는지는 미검증입니다. Linux Sober의 바이옴 로그 감시는 아직 연결하지 않았습니다. 위 Windows DUA 검사와 과거 Linux 앱 시작 기록은 새 Linux 게임 연결의 검증 결과와 구분합니다.

### Apple 빌드 확인

Mac은 기존 데스크톱 화면을 사용합니다. Mac 빌드에는 macOS와 Xcode가 필요합니다. 사용자 요청에 따라 iOS는 앞으로의 빌드·실행 검사 대상에서 제외했습니다.

- macOS Intel·Apple Silicon 통합 앱 빌드에는 `npm run build:macos`를 사용합니다. 두 Apple Darwin Rust 타깃이 필요합니다.
- `.github/workflows/apple.yml`은 macOS 통합 앱을 빌드하고 시작을 확인합니다.
- `.github/workflows/alpha.yml`은 `dev`에서도 Android·Windows 빌드를 확인합니다. 이 브랜치의 결과는 작업 실행의 첨부 파일로 제공하며 릴리스 갱신은 main에서 수행합니다.

검증 코드 커밋은 `0f47344`입니다. GitHub 실행기에서 Windows 실행 파일과 macOS Intel·Apple Silicon 통합 앱을 빌드하고 실제 시작을 확인했습니다. Android ARM64 APK 빌드와 순수 Java 터치 타임라인 회귀 검사도 통과했습니다. Rust 코어 테스트는 16개 모두 통과했습니다.

- [Android·Windows 검사 및 빌드 파일](https://github.com/XxErone2xX/Acrux-app/actions/runs/42947345168)
- [Mac 검사 및 통합 앱](https://github.com/XxErone2xX/Acrux-app/actions/runs/42947345249)

PC의 Python 모드 실행·복구 자동화와 Mac 게임 화면 캡처·입력 연결은 이식 전입니다. Windows·Mac에서 시작 검사를 통과했으며 Roblox 안의 실제 동작은 검증 전입니다.

### Android 메모리 측정

UI 재현 전 Android 16 (SM-S931N)에서 각 앱을 화면에 띄워 PSS를 세 번 측정한 중앙값입니다. Tauri 빌드는 앱과 전용 WebView 렌더러를 합산했습니다.

| 상태 | 기존 1.1.0 | Tauri 기본 렌더링 | Tauri 소프트웨어 렌더링 |
| --- | ---: | ---: | ---: |
| 화면 대기 | 118.7 MiB | 176.6 MiB | 117.6 MiB |
| 화면 캡처 대기 | 126.6 MiB | 187.5 MiB | 125.0 MiB |

미리보기의 Android 화면에는 소프트웨어 렌더링을 적용했습니다. 새 UI와 Roblox에서 낚시 중인 메모리는 별도 검증이 필요합니다.

## 이전 미리보기 검사 기록

- DUA 도입 전 미리보기에서 `npm run build`, `cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --lib` (16개), Windows 타깃 코드 검사, Linux 데스크톱 빌드·시작 확인을 통과했습니다.
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

PC 런타임 의존성 변경에 따라 Python 런타임 번호를 5로 올렸습니다. 회귀 검사는 `npm run test:desktop` (3개)과 `python -m unittest discover -s tests -v` (5개)이며 Windows·Mac 빌드 검사에 포함했습니다.

최종 제거 코드 `0f47344`에서 화면·설정 회귀 검사 3개, Python 설정·알림·모드 API 검사 5개, Rust 코어 검사 16개를 Windows·Mac 실행기에서 통과했습니다. 두 앱의 빌드와 실제 시작을 확인했으며 Mac 파일은 Intel·Apple Silicon 통합 형식도 검사했습니다. Android 빌드도 통과했습니다.

일반 모드의 바이옴 이벤트는 기존 3인자 형식으로 전달하고 마지막 인자는 고정된 `False`로 처리합니다. 예전 설정·모드의 제거 처리, 이 API 호환 처리와 테스트에만 과거 기능의 식별자가 필요합니다.

화면 검사는 호출을 대체한 브라우저에서 연동 항목의 부재, 공통 설정 저장, 내 서버 포션 목록 편집을 확인했습니다. Linux의 실제 Tauri 앱 시작과 기존 Python 앱의 백그라운드 작업 없는 초기화도 확인했습니다. Roblox 안의 동작과 기존 Python 배포 패키지의 전체 빌드 검증은 남아 있습니다.
