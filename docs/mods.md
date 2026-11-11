# Acrux 모드 · 플러그인 만들기 (PC)

- **모드**: 설정 탭 · 화면이 생기는 큰 기능
- **플러그인**: 화면 없이 뒤에서 이벤트에 반응하는 작은 기능

설치 위치: `%LOCALAPPDATA%\AcruxMacro\data\mods\<id>\` (추가 기능 탭 → 모드 · 플러그인 → [모드 파일 설치])
모드 파일 (`.acxm`) = 아래 폴더를 zip 으로 묶은 것 · `python mods/build.py <id>` 로 만듦

## 폴더

```
<id>/
  mod.json      {"id": "<id>", "name": "이름", "version": "1.0.0", "type": "mod" | "plugin",
                 "author": "…", "description": "…", "acrux": "3.0.0", "entry": "main",
                 "ui": {"html": "ui.html", "js": "ui.js", "css": "ui.css"}}   ← ui 는 모드만 · 다 선택
  __init__.py   (빈 파일)
  main.py       def setup(ctx): …
  ui.html · ui.js · ui.css (모드만)
```

- `id`: 영문 소문자로 시작 · 영문 소문자 · 숫자 · `_` (2~31자) · 폴더 이름과 같음
- 파이썬은 `acrux_mods.<id>` 패키지로 불러옴 → 모드 안 파일끼리는 `from . import x`
- 본체 모듈 (`core` · `macro` · `popping` · `biome` · `rejoin` · `steps` …) 과 Acrux 런타임 라이브러리 (`numpy` · `cv2` · `mss` …) 를 그대로 `import` 가능

## setup(ctx)

| | |
|---|---|
| `ctx.log(글, 색)` | 로그 (색: `g` · `y` · `r` · `c` · `d`) |
| `ctx.api("이름")(함수)` | 화면에서 `api('이름', {...})` 로 부르는 함수 (이름: 영문 소문자 · `_`) |
| `ctx.on("이벤트", 함수)` | 아래 이벤트 |
| `ctx.config_keys(기본값, 정리)` | `config.json` 맨 위에 쓸 키 · 화면 `queueSave` 로 저장 가능해짐 |
| `ctx.settings` · `ctx.save()` | 모드 자기 설정 (`config.json` → `mod_settings.<id>`) |
| `ctx.bridge` · `ctx.core` | 본체 (고급 · 매크로 · 복귀 · Play 클릭 등을 직접) |

이벤트 (`mods.Hooks`): `busy(kind)` → True 면 매크로가 쉼 · `active()` · `state(out)` · `poll(out, since)` · `stats()` · `ingame(path, reason)` · `play_fail(reason)` · `after_return(head, wait)` · `biome(prev, found)` · `macro(on)` · `stop()` · `ready()` · `shutdown()`

## 화면 (모드)

- `ui.html`: 화면 (`<section class="page" id="page-<키>">` …) · `data-mod-into="선택자"` 가 붙은 요소는 그 안으로 (예: 메인 타일 → `#mains`)
- `ui.js`: 본체 `app.js` 의 함수 · 값을 그대로 씀 (`api` · `config` · `queueSave` · `toast` · `$` · `MENU` · `TABS` · `Quick` · `Tutorial` · `I18N`) + `Mods`
  - `Mods.addTab(키, 이름, 앞 탭)` · `Mods.addMenu({key, tab, icon, color, title, desc}, …)`
  - `Mods.on('init' | 'poll' | 'summary' | 'tiles' | 'active' | 'ret', 함수)`
  - `Quick.add(키, {render, sync, click, reset, wide})` (메인 타일 ▾) · `Tutorial.register(kit => [튜토리얼])` · `I18N.add([[한국어, English, 日本語], …])`

모드는 Acrux 와 같은 권한으로 도는 프로그램이라, 믿을 수 있는 곳에서 받은 것만 설치하세요.
