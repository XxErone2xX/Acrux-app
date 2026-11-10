# Acrux macro

Sol's RNG 용 Acrux macro — PC (윈도우) · 모바일 (안드로이드)

## Tauri 재작성 미리보기

Android와 Windows는 기존 앱별 화면을 따릅니다. Android의 TypeScript 화면은 기존 모바일 탭과 카드를 재현하고, Windows는 기존 HTML·CSS·화면 동작을 가져와 TypeScript를 통해 Tauri에 연결합니다. 두 앱은 `src-tauri/`의 Rust 낚시·이동·바이옴 엔진을 공유합니다. 현재 이식 기준은 모바일 1.4.1과 PC 2.5.4 (`02977ec`)입니다.

- `npm ci` 후 `npm run build`로 화면을 빌드합니다.
- Windows에서는 `npm run tauri -- dev`로 실행하거나 `npm run tauri -- build`로 패키징합니다.
- Android에서는 JDK 17, Android SDK 36, NDK와 Rust의 `aarch64-linux-android` 타깃을 준비한 뒤 `npm run tauri -- android build --apk --target aarch64 --debug`를 실행합니다. 테스트 패키지 `com.acrux.next`는 기존 앱과 별도로 설치됩니다.

미리보기에는 낚시와 이동·판매, 바이옴 감시·디스코드 알림 흐름이 들어 있습니다. Android에서는 조작 창에서 조이스틱과 버튼 위치를 직접 지정하고 방향 패드의 실제 누름·이동·뗌 시각으로 길을 기록합니다. 이전 미리보기의 0.5초 단위 기록도 읽을 수 있으며 새 타이밍을 적용하려면 길을 다시 기록해야 합니다. Windows에서는 기존 이동 메뉴에서 지점 위치와 도착 시간을 저장합니다. 두 앱 모두 수동으로 지정한 판매 위치를 사용합니다.

Android의 바이옴 기능은 앱 내부 무선 디버깅 페어링과 로그 수집을 사용합니다. 로그 재연결 시 이어 읽기와 2분 정체 감지, 웹후크별 순차 전송·429 재시도를 포함합니다. 사용자 수는 `https://acruxmacro.com/api/`에서 읽습니다.

판매 자동 보정, 조이스틱 자동 검색, 나머지 PC 전용 자동화, 기존 설정 이전, 배포 업데이트는 아직 이식 전입니다. Roblox 안에서 이동과 판매가 동작하는지는 별도 검증이 필요합니다. 아래 다운로드 링크는 기존 버전을 가리킵니다.

### Apple 빌드 확인

Mac은 기존 데스크톱 화면을, iOS는 모바일 화면을 사용합니다. Apple 빌드에는 macOS와 Xcode가 필요합니다. iOS의 Xcode 프로젝트는 Mac에서 `npm run init:ios`로 생성합니다.

- macOS Intel·Apple Silicon 통합 앱 빌드에는 `npm run build:macos`를 사용합니다. 두 Apple Darwin Rust 타깃이 필요합니다.
- iOS 실기기용 무서명 빌드에는 `npm run init:ios` 다음 `npm run build:ios`를 사용합니다. 설치·배포용 서명은 별도 설정이 필요합니다.
- Apple Silicon Mac의 iOS 시뮬레이터용 빌드에는 `npm run build:ios:simulator`를 사용합니다. Intel Mac에서는 `npm run tauri -- ios build --target x86_64 --ci --no-sign`을 사용합니다.
- `.github/workflows/apple.yml`의 수동 실행은 macOS 통합 앱, iOS 실기기용 무서명 IPA와 호스트 아키텍처에 맞는 시뮬레이터 앱을 빌드합니다.

현재 Apple 화면 캡처·게임 입력 연결은 이식 전입니다. Linux에서 Apple Silicon·Intel Mac용 Rust 코드 검사를 통과했습니다. iOS 대상 검사는 Apple SDK 탐색 도구인 xcrun 부재로 중단됐습니다. macOS 앱 실행과 iOS 시뮬레이터 실행은 Mac 실행기에서 추가 확인이 필요합니다.

### Android 메모리 측정

UI 재현 전 Android 16 (SM-S931N)에서 각 앱을 화면에 띄워 PSS를 세 번 측정한 중앙값입니다. Tauri 빌드는 앱과 전용 WebView 렌더러를 합산했습니다.

| 상태 | 기존 1.1.0 | Tauri 기본 렌더링 | Tauri 소프트웨어 렌더링 |
| --- | ---: | ---: | ---: |
| 화면 대기 | 118.7 MiB | 176.6 MiB | 117.6 MiB |
| 화면 캡처 대기 | 126.6 MiB | 187.5 MiB | 125.0 MiB |

미리보기의 Android 화면에는 소프트웨어 렌더링을 적용했습니다. 새 UI와 Roblox에서 낚시 중인 메모리는 별도 검증이 필요합니다.

## 다운로드

| | 받기 | 업데이트 |
| --- | --- | --- |
| **PC (윈도우)** | [**Acrux.exe 받기**](https://github.com/XxErone2xX/Acrux-app/releases/latest/download/Acrux.exe) | 켤 때마다 자동 |
| **모바일 (안드로이드 8.0+)** | [**Acrux-Mobile.apk 받기**](https://github.com/XxErone2xX/Acrux-app/releases/download/mobile-latest/Acrux-Mobile.apk) | 앱을 켤 때마다 자동 (V0.7.0 부터) |

- 두 링크 모두 늘 최신 버전으로 받아짐 (링크가 바뀌지 않음)
- 바뀐 점: [PC 릴리스](https://github.com/XxErone2xX/Acrux-app/releases/latest) · [모바일 릴리스](https://github.com/XxErone2xX/Acrux-app/releases/tag/mobile-latest)

### 모바일 설치

1. 위 링크로 APK 를 받아서 설치 (브라우저 · 파일 앱에 '알 수 없는 앱 설치'를 허용)
2. V0.6.x 이하가 깔려 있으면 **한 번만** 지우고 설치 (V0.7.0 부터 서명이 고정돼서 그 뒤로는 덮어쓰기 · 자동 업데이트)
3. 앱 안 '준비' 순서대로: 접근성 권한 → 화면 캡처 → (이동 · 판매를 쓸 때) 로블록스 이동 방식을 조이스틱(클래식)으로 · 루팅 · Shizuku 필요 없음

## Download

| | Get it | Updates |
| --- | --- | --- |
| **PC (Windows)** | [**Download Acrux.exe**](https://github.com/XxErone2xX/Acrux-app/releases/latest/download/Acrux.exe) | automatic on every launch |
| **Mobile (Android 8.0+)** | [**Download Acrux-Mobile.apk**](https://github.com/XxErone2xX/Acrux-app/releases/download/mobile-latest/Acrux-Mobile.apk) | automatic on every app launch (from V0.7.0) |

Both links always point to the latest version.

---

배포 · 빌드 방법 (개발용): [RELEASE.md](RELEASE.md)
