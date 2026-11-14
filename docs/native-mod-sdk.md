# DUA 공통 기반과 모드 SDK

DUA는 기본 매크로와 모드가 함께 쓰는 실행 기반이다. 상태·로그를 읽는 Headless 경로와 픽셀·OCR을 읽는 Screenshot 경로, 직렬 입력·작업 취소·이동 상태는 같은 Rust 실행기가 소유한다. 모드를 위해 별도 캡처기나 입력 엔진을 만들지 않는다. Windows·Linux·macOS·Android에서 같은 공개 기능 목록을 제공하고, 연결 준비와 전경/백그라운드 조건은 실행 상태로 알려준다.

## 공통 호출

JavaScript 모드는 `ctx.dua`, 앱의 화면 코드는 `window.AcruxDUA`로 같은 기반 기능을 호출한다. 기존 데스크톱 Python 모드는 `import dua` 또는 `ctx.dua`를 사용한다. 전송 창구는 기존 `automation_action`의 `sdk_call`이며 `module: "dua"`, `method`, `args`, `kwargs`를 전달한다.

| 호출 | 동작 |
| --- | --- |
| `capabilities()` | 공통 API 버전·기능·목적지·Headless/Screenshot 경로 목록 |
| `status()`, `logs()` | 화면 캡처 없이 상태·기록 읽기 |
| `start(["mfish", …])`, `stop()` | 기본 앱과 같은 매크로 시작·전체 정지 |
| `go("sell")`, `mark("sell")`, `cancel()` | 기존 이동·장소 등록·진행 요청 취소 |
| `observe(rect?)` | 같은 프레임의 BGRA 픽셀·OCR·촬영 시각·입력 기한·실행 세대 |
| `tap(x,y,ms)`, `gesture(value)`, `text(value,submit)` | 게임 기준 좌표의 실제 입력 완료를 기다림 |

`mark()`는 카메라를 자동으로 위로 올리고 시야를 넓힌 뒤 현재 서 있는 장소를 지형 지도에 저장한다. 시설 표식이나 플레이어 이름은 필요하지 않으며, 저장 완료 또는 취소까지 기다린다. `go()`는 화면 중심의 현재 위치와 누적 관측 지도로 경로를 구한다. 목적지가 화면 밖에 있어도 연결된 지도에 있으면 중간 지점을 따라가며, 아직 연결되지 않은 영역은 확인된 통로를 관측하면서 탐색한다. 좌표를 모르는 지형에 임의의 경로나 점프를 만들지 않는다.

JavaScript에서는 `key(name,ms)`, `open(url)`도 제공한다. 원시 호출과 Python 이름은 `key_tap`, `open_link`다. 플랫폼 공통 이동은 `go` 또는 `gesture`의 `stick`을 쓴다. 데스크톱은 방향을 WASD/Space로, Android는 조이스틱/점프로 전달한다. 임의의 PC 키가 Android에도 있다는 뜻은 아니다.

`observe([left,top,right,bottom])`의 영역과 OCR 박스는 게임 전체 기준 0~1 좌표다. 결과 `image`에는 `pixels`(BGRA 바이트), `width`, `height`, 잘라낸 위치 `origin:[x,y]`가 있다. `words`는 `[text,cx,cy,width,height]` 배열이다. `capturedAt`·`inputDeadline`은 앱 단조 시계의 초이고 `generation`은 실행 세대다. AI나 외부 모델은 사용하지 않는다.

## 네 OS 공통 JavaScript 모드

`.acxm` ZIP에 아래 두 파일을 넣는다. 설치·켜기·끄기·삭제·설정 항목은 PC와 모바일의 모드 화면에서 제공한다. 코드 변경과 활성화 변경은 앱을 다시 켜면 적용한다.

```json
{"id":"coast_helper","name":"Coast helper","version":"1.0.0","runtime":"javascript","entry":"main","type":"mod"}
```

`main.js`:

```js
export function setup(ctx) {
  ctx.ui([
    {type: "button", label: "바닷가로 이동", run: () => ctx.dua.go("coast")},
    {type: "button", label: "정지", run: () => ctx.dua.stop()}
  ]);
}
```

복사해서 시작할 수 있는 전체 예제는 `examples/dua-coast-helper/`다. 폴더의 `mod.json`과 `main.js`를 ZIP으로 묶어 `.acxm` 확장자로 설치하면 된다. 같은 파일로 네 플랫폼에서 이동·장소 기억·Headless 상태·스크린샷 관측·정지를 호출한다.

설정 항목은 `text`·`number`·`checkbox`, 실행 항목은 `button`, 설명은 `info`다. 모든 기기에서 같은 항목을 표시한다. `ctx.settings`와 `await ctx.save(patch)`는 모드 ID별 설정을 쓰며 다른 기능 설정을 덮어쓰지 않는다. `ctx.on("ready"|"poll"|"stop"|"shutdown", callback)`은 공통 이벤트다. 상태 관측은 화면 탭의 표시 여부에 종속되지 않으며 각 모드의 비동기 관측은 겹쳐 실행하지 않는다. 타이머의 정확한 주기는 OS/WebView에 따라 달라질 수 있으므로 정밀 낚시·입력 타이밍은 공통 Rust 실행기에 맡긴다.

패키지의 JavaScript는 앱 권한으로 실행한다. 앱 파일·설정·입력 접근이 가능한 설치 코드이며 별도의 격리 환경을 제공하지 않는다. JavaScript는 ES 모듈로 작성하고 의존 파일은 패키지 내부 상대 경로로 가져온다. 공개 타입은 `frontend/dua.ts`와 `frontend/portable-mods.ts`의 `ModContext`·`ModField`다.

모드 파일 주소는 Tauri의 기존 커스텀 프로토콜을 사용한다. Windows·Android의 HTTP 주소와 macOS·Linux의 스킴 주소 차이는 호스트에서 처리한다. [Tauri 커스텀 프로토콜 설정](https://v2.tauri.app/reference/config/)

## 기존 데스크톱 Python 모드

`dev`의 데스크톱 앱은 정식 Python 앱의 `src/mods.py`를 확장 SDK로 재사용한다. 매크로 본체와 게임 입력·캡처는 Rust에 있으며, Python 확장은 별도 실행기에서 동작한다. Android에는 Python 모드 화면이나 실행기를 추가하지 않는다.

## 설치와 화면

기존 `.acxm`, `.acrux`, `.zip` 패키지를 모드 화면에서 선택한다. `mod.json`과 `setup(ctx)` 진입점은 기존 형식을 유지한다. `ctx.api`, `ctx.on`, `ctx.config_keys`, `ctx.settings`, `ctx.save`, `ctx.bridge`, `ctx.core`도 같은 SDK를 사용한다. 설치·활성화·삭제 후에는 다시 시작해야 적용된다.

패키지는 압축 전후 각각 30MiB로 제한하고, 폴더 밖 경로·심볼릭 링크·퇴역한 sniping 패키지를 거부한다. 기존 설치를 교체하기 전에 새 패키지와 진입점을 검사한다. 모드의 화면 파일은 앱의 `acruxmod` 경로로 제공한다. 설치한 Python은 사용자 권한으로 실행되는 확장 코드다.

시스템 Python 3.10 이상이 있으면 사용한다. 없으면 공식 python-build-standalone의 해당 OS·아키텍처 런타임과 SHA256을 확인한 뒤 앱 데이터 폴더에 준비한다. 모드가 추가 Python 패키지를 요구하면 해당 import 오류를 모드 목록에 표시하며, 실행 성공으로 처리하지 않는다.

## 이벤트와 설정

기존 `ready()`는 모드를 모두 불러온 뒤 한 번 실행한다. 실행 관련 이벤트는 크기가 제한된 단일 대기열을 통해 처리한다. 확장이 로그를 남길 때 동일한 로그 이벤트를 재귀 전송하지 않는다. 게임 처리의 잠금을 잡은 채 Python 응답을 기다리지 않는다.

`state(out)`과 `poll(out, since)`의 `out` 변경은 실제 PC 화면 응답에 반영한다. `stats()`가 반환한 숫자 사전은 이번 실행 통계에 합친다. `macro(on)` 및 `start`·`stop` 이벤트는 `core.STOP`에도 반영된다. 앱 종료에서는 실행 중인 native 작업을 먼저 중지한 뒤 확장의 `stop/shutdown`을 최대 3초 기다리고 자식 실행기를 종료한다. 종료 훅은 설정 저장이 가능하지만 새 게임 입력은 거절한다. 실제 게임의 바이옴·입장·복귀 이벤트는 공통 제어기에서 전달한다.

설정 변경은 네이티브 저장소에 기록된다. 실행 중 다른 작업이 설정을 사용하고 있으면 저장 오류를 돌려준다. PC·모바일 기본 UI는 설정을 바꾸기 전에 중지하고, 아직 실행 의도가 남아 있을 때만 한 번 재시작한다. 사용자의 정지는 저장 완료보다 우선한다.

## 네이티브 입력·관측 창구

확장 코드의 `core`·`macro` 호출은 공통 네이티브 입력 소유자에게 전달된다. 이전 Python 엔진의 호스트 키보드 입력이나 전경 캡처를 다시 실행하지 않는다. 지원하지 않는 구형 함수를 호출하면 구체적인 오류를 돌려준다. 전역 창 핸들 조회, `focus`, `bring_front` 등 호스트의 전경 전환은 거절한다.

| 함수 | 입력과 반환 |
| --- | --- |
| `tap(x, y, 40)` | 게임 화면의 0~1 좌표. 실제 입력 완료 확인 후 반환 |
| `click(x, y, ...)` | 게임 화면 기준 픽셀 좌표. 호스트 바탕화면 좌표가 아님 |
| `key_tap(name, 40)` | 알파벳·숫자·Space·Enter·Escape·Tab·Backspace 등 지원 키 |
| `gesture(value)` | `kind`가 `tap`·`drag`·`pinch`·`stick`인 공통 제스처. 좌표는 게임 기준 0~1, 입력은 1~1000ms |
| `text(value, submit=False)` | 게임에 문자열 입력. `type_text`·`paste`도 같은 네이티브 경로 |
| `open_link(url)` | 허용된 Roblox 입장 주소 실행 |
| `ocr_region(rect=None)` | 새 관측의 글자를 문자열로 반환 |
| `ocr_boxes(rect=None)` | `[text, center_x, center_y, width, height]` 목록. 좌표는 게임 기준 0~1 |
| `grab([left, top, width, height])` | 게임 기준 픽셀 영역의 `(BGRA bytes, width, height)` 튜플 |
| `ocr_engine()` / `ocr_engine_name()` | 실제 기기 OCR 엔진 이름 |
| `client_rect()` | 관측한 게임의 `[0, 0, width, height]` |

OCR 영역은 `[left, top, right, bottom]`의 0~1 좌표다. 캡처 영역은 최대 300만 픽셀이다. 캡처 응답의 내부 base64 표시는 확장 실행기가 원래 BGRA 튜플로 복원하므로 모드 코드에서 처리할 필요가 없다.

네이티브 요청은 실행 세대와 실제 입력 완료를 검사한다. 네이티브 응답 대기는 25초, Python의 개별 RPC 대기는 30초다. 타임아웃·중지·창 변경으로 취소된 입력은 성공으로 재시도하지 않는다.

## 업데이트와 검사

개발 앱의 업데이트는 정식 앱의 `latest`·`mobile-latest`와 분리한 `dev-alpha`만 사용한다. dev의 Linux·Windows·Android·macOS 빌드가 모두 성공한 뒤 한 게시 작업에서 실제 자산 및 `.sha256`을 올린다. main의 기존 `alpha`는 이 앱의 업데이트 대상이 아니다. 앱은 SHA256을 검사하고, Android에서는 설치 패키지명과 서명도 검사한다. macOS는 압축 자산과 설치된 실행 파일의 해시를 함께 보관해 같은 업데이트를 반복 제안하지 않는다.

현재 추가한 검사는 실제 `setup(ctx)`, 등록 API, 이벤트, 설정 왕복, BGRA 응답 복원과 패키지 교체·경로 검증을 다룬다. 실제 확장 설치·네트워크 다운로드·각 OS 설치 화면은 이 단위 검사의 완료만으로 실기 검증됐다고 보지 않는다.
