'use strict';
// ================================================================ 언어 (한국어 / English / 日本語)
// 화면에 나오는 한국어 글자를 그대로 열쇠로 써서 번역함 — 화면 코드는 한국어로 그대로 두고,
// 화면이 바뀔 때마다(MutationObserver) 새로 생긴 글자를 자동으로 번역
//  · 문장 안에 <b> 같은 태그가 섞인 <p>/<li> 는 통째로 번역 (HTML 그대로 열쇠)
//  · 숫자·이름이 들어가는 글자는 R(정규식) 로 번역 — $1, $2 … 자리에 들어간 글자도 다시 번역
//  · 사전에 없는 글자는 ' · ', ' — ', ' → ', ' + ' 로 나눠서 조각마다 번역 (끝까지 없으면 한국어 그대로)
const I18N = (() => {
  const LANGS = { ko: '한국어', en: 'English', ja: '日本語' };
  const HANGUL = /[가-힣]/;
  let lang = 'ko';

  // ---------------------------------------------------------------- 사전: [한국어, English, 日本語]
  const E = [
    // 메인
    ['중지됨', 'Stopped', '停止中'],
    ['감지', 'Detected', '検知'],
    ['시작', 'Start', '開始'],
    ['중지', 'Stop', '停止'],
    ['설정', 'Settings', '設定'],
    ['더 보기', 'More', 'もっと見る'],
    ['접기', 'Hide', '閉じる'],
    ['로블록스 클라이언트가 감지되지 않았습니다.', 'Roblox client not detected.', 'Robloxクライアントが見つかりません。'],
    ["Sol's RNG에 접속하시겠습니까?", "Join Sol's RNG?", "Sol's RNGに参加しますか？"],
    ['닫기', 'Close', '閉じる'],
    ['설정 자동 저장', 'Settings save automatically', '設定は自動保存'],
    ['Esc: 뒤로', 'Esc: Back', 'Esc: 戻る'],
    ['뒤로', 'Back', '戻る'],
    ['바이옴 매크로 설정', 'Biome macro settings', 'バイオームマクロ設定'],
    ['바이옴 알림 · 디스코드 웹후크', 'Biome alerts · Discord webhook', 'バイオーム通知・Discord Webhook'],
    ['디스코드 감지 설정', 'Discord detection settings', 'Discord検知設定'],
    ['감시할 서버·채널, 필터, 감지 기록', 'Servers·channels to watch, filter, log', '監視するサーバー・チャンネル、フィルター、検知履歴'],
    ['오토 팝핑 매크로 설정', 'Auto popping macro settings', 'オートポッピングマクロ設定'],
    ['게임 접속 후 Play 버튼 자동 클릭', 'Auto-clicks Play after joining', '参加後にPlayボタンを自動クリック'],
    ['매크로 복귀 설정', 'Macro return settings', 'マクロ復帰設定'],
    ['바이옴이 끝나면 내 서버로 복귀', 'Returns to your server when the biome ends', 'バイオーム終了後に自分のサーバーへ復帰'],
    ['준비 중', 'Coming soon', '準備中'],
    ['바이옴', 'Biome', 'バイオーム'],
    ['스나이프', 'Snipe', 'スナイプ'],
    ['매크로', 'Macro', 'マクロ'],
    ['곧 추가됩니다', 'Coming soon', '近日追加'],
    // 매크로 탭
    ['매크로 기능 설정', 'Macro feature settings', 'マクロ機能設定'],
    ['내 서버에서 돌릴 기능 켜기 · 끄기', 'Turn on/off features that run in your server', '自分のサーバーで動かす機能のオン・オフ'],
    ['매크로 기준 위치 설정', 'Macro base position settings', 'マクロ基準位置設定'],
    ['기능마다 버튼 위치 · 영역 지정', 'Button positions · areas for each feature', '機能ごとのボタン位置・範囲を指定'],
    ['통계 보기', 'View stats', '統計を見る'],
    ['이번 실행 · 올타임 기록', 'This run · All-time records', '今回の実行・オールタイム記録'],
    ['레어 바이옴 자동 팝핑', 'Rare biome auto popping', 'レアバイオーム自動ポッピング'],
    ['내 서버에서', 'In your server', '自分のサーバーで'],
    ['상인 자동 구매', 'Merchant auto buy', '商人自動購入'],
    ['마리 · 제스터', 'Mari · Jester', 'マリ・ジェスター'],
    ['포션 자동 제작', 'Potion auto craft', 'ポーション自動作成'],
    ['Auto Crafted 알림', 'Auto Crafted notice', 'Auto Crafted通知'],
    ['메모리 매치', 'Memory Match', 'メモリーマッチ'],
    ['오토 메모리 매치', 'Auto Memory Match', 'オートメモリーマッチ'],
    // 매크로 탭 · 자동 낚시
    ['매크로 버튼과 아래 켜기가 켜져 있으면, 지금 켜진 로블록스(내 서버)의 낚시 자리에서 계속 낚시', 'While the Macro button and the switch below are on, keeps fishing at the fishing spot in the Roblox that is open now (your server)', 'マクロボタンと下のスイッチがオンの間、今開いているRoblox（自分のサーバー）の釣り場で釣り続ける'],
    ['정지: F7 (매크로 꺼짐)', 'Stop: F7 (turns Macro off)', '停止: F7（マクロがオフになる）'],
    ['상태 확인', 'Check status', '状態確認'],
    ['세부 설정', 'Fine-tuning', '詳細設定'],
    ['매크로 버튼이 켜져 있는 동안 계속 낚시', 'Keeps fishing while the Macro button is on', 'マクロボタンがオンの間、釣り続ける'],
    ['레어 바이옴이 뜨면 잠깐 멈추고 팝핑 후 이어감', 'pauses for rare biome popping, then continues', 'レアバイオームが出たら一時停止してポッピング後に再開'],
    ['16:9 로블록스 창 기준 기본 위치를 한 번에 채움', 'Fills default positions for a 16:9 Roblox window at once', '16:9のRobloxウィンドウ基準の既定位置を一度に入力'],
    ['안 맞으면 아래에서 직접 지정', 'set them below if they do not fit', '合わなければ下で直接指定'],
    ['16:9 적용', 'Apply 16:9', '16:9を適用'],
    ['16:9 템플릿 적용', '16:9 template applied', '16:9テンプレートを適用'],
    ['Fish 버튼 위치', 'Fish button position', 'Fishボタンの位置'],
    ['Fish / Exit 버튼 가운데 (같은 자리)', 'Center of the Fish / Exit button (same spot)', 'Fish / Exitボタンの中央（同じ場所）'],
    ['결과창 X 위치', 'Result window X position', '結果ウィンドウXの位置'],
    ['낚시 결과창 오른쪽 위 X', 'X at the top right of the fishing result window', '釣り結果ウィンドウ右上のX'],
    ['결과창 제목 위치', 'Result title position', '結果タイトルの位置'],
    ['선택', 'Optional', '任意'],
    ['제목 색으로 성공 / 쓰레기 / 실패 구분', 'tells success / junk / fail apart by title color', 'タイトルの色で成功 / ゴミ / 失敗を区別'],
    ['릴링 바 영역', 'Reel bar area', 'リールバーの範囲'],
    ['위쪽 바(청록 막대 · 색 구간이 있는 바)만 딱 맞게 드래그', 'Drag tightly around the upper bar only (the one with the teal bar · colored zone)', '上のバー（青緑のバー・色の区間があるバー）だけをぴったりドラッグ'],
    ['◇ 표시는 자동으로 찾음', 'the ◇ marker is found automatically', '◇マークは自動で探す'],
    ['위쪽 바(파란 막대 · Ready! 가 뜨는 바)만 딱 맞게 드래그', 'Drag tightly around the upper bar only (the blue bar where Ready! shows)', '上のバー（青いバー・Ready! が出るバー）だけをぴったりドラッグ'],
    ['기능 켜기 · 끄기', 'Features on / off', '機能のオン・オフ'],
    ['쓸 기능을 한 번에', 'All features at once', '使う機能を一度に'],
    ['전부 끄기', 'All off', 'すべてオフ'],
    ['전부 켜기', 'All on', 'すべてオン'],
    ['켜 둔 기능만 메인 화면의 매크로 버튼을 켰을 때 동작', 'Only the features turned on here run when the Macro button on the main screen is on', 'オンにした機能だけが、メイン画面のマクロボタンをオンにしたときに動作'],
    ['위치는 매크로 기준 위치 설정에서 지정', 'Positions are set in Macro base position settings', '位置はマクロ基準位置設定で指定'],
    ['버튼 6개 · OCR 영역', '6 buttons · OCR area', 'ボタン6個・OCR範囲'],
    ['Fish 버튼 · 릴링 바 · 결과창', 'Fish button · reel bar · result', 'Fishボタン・リールバー・結果'],
    ['이동 기능의 출발점', 'Start point for movement', '移動機能の出発点'],
    ['스나이프 탭 위치 가져오기', 'Copy from Snipe tab', 'スナイプタブの位置を取り込む'],
    ['로블록스를 켜 두고 각 항목의 [위치 지정] → 로블록스 화면에서 클릭 (영역은 드래그)', 'With Roblox open, press [Set position] on each item → click in Roblox (drag for areas)', 'Robloxを開いたまま各項目の[位置指定] → Roblox画面でクリック（範囲はドラッグ）'],
    ['위치는 로블록스 창 기준이라 창 크기를 바꾸면 다시 지정', 'positions follow the Roblox window, so set them again if you resize it', '位置はRobloxウィンドウ基準なので、サイズを変えたら指定し直す'],
    ['낚시 자리에서 낚시 창을 띄워 두고 지정', 'Set these at your fishing spot with the fishing window open', '釣り場で釣りウィンドウを出したまま指定'],
    ['릴링 바와 결과창은 한 번 낚아서 화면에 떠 있을 때 지정', 'set the reel bar and result while they are on screen after casting once', 'リールバーと結果は一度釣って画面に出ているときに指定'],
    ['[상태 확인] 으로 제대로 읽는지 확인', 'use [Check status] to see if they are read correctly', '[状態確認]で正しく読めるか確認'],
    ['버튼 위치 · OCR 영역', 'Button positions · OCR area', 'ボタン位置・OCR範囲'],
    ['매크로 기준 위치 설정 → 레어 바이옴 자동 팝핑 에서 지정', 'Set in Macro base position settings → Rare biome auto popping', 'マクロ基準位置設定 → レアバイオーム自動ポッピングで指定'],
    ['딜레이 · 이름 일치율', 'Delays · name match', 'ディレイ・名前一致率'],
    ['오토 팝핑과 같음', 'Same as auto popping', 'オートポッピングと同じ'],
    ['버튼 위치 · 릴링 바 영역', 'Button positions · reel bar area', 'ボタン位置・リールバー範囲'],
    ['매크로 기준 위치 설정 → 자동 낚시 에서 지정', 'Set in Macro base position settings → Auto fishing', 'マクロ基準位置設定 → 自動釣りで指定'],
    ['내 서버에서 레어 바이옴이 뜨면 포션 사용', 'Uses potions when a rare biome starts in your server', '自分のサーバーでレアバイオームが来たらポーションを使用'],
    ['제자리 낚시 (판매와 이동은 다음 업데이트)', 'Fishing in place (selling and movement in a later update)', 'その場で釣り（売却と移動は次のアップデート）'],
    ['16:9 로블록스 창(1920x1080 전체 화면 등) 기준 위치를 한 번에 채움', 'Fills positions for a 16:9 Roblox window (e.g. 1920x1080 full screen) at once', '16:9のRobloxウィンドウ（1920x1080全画面など）基準の位置を一度に入力'],
    ['안 맞는 건 아래에서 직접 지정', 'set any that are off below', '合わないものは下で直接指定'],
    ['스나이프 탭 오토 팝핑 위치를 가져옴', 'Copied the auto popping positions from the Snipe tab', 'スナイプタブのオートポッピング位置を取り込みました'],
    ['스나이프 탭 오토 팝핑에 지정된 위치 없음', 'No auto popping positions set in the Snipe tab', 'スナイプタブのオートポッピングに位置がありません'],
    ['기능 전부 켜짐', 'All features on', 'すべての機能 オン'],
    ['기능 전부 꺼짐', 'All features off', 'すべての機能 オフ'],
    ['로블록스 화면에서 OCR 영역 드래그', 'Drag the OCR area in Roblox', 'RobloxでOCR範囲をドラッグ'],
    ['로블록스 화면에서 릴링 바 영역 드래그', 'Drag the reel bar area in Roblox', 'Robloxでリールバーの範囲をドラッグ'],
    ['릴링 바 영역 저장', 'Reel bar area saved', 'リールバーの範囲を保存'],
    ['이번 실행 기록', 'This run', '今回の記録'],
    ['성공 · 쓰레기 · 실패 · 인벤토리 가득', 'Success · Junk · Fail · Inventory full', '成功・ゴミ・失敗・インベントリ満杯'],
    ['자동 낚시 켜짐', 'Auto fishing on', '自動釣り オン'],
    ['자동 낚시 꺼짐', 'Auto fishing off', '自動釣り オフ'],
    ['입질 최대 대기', 'Max bite wait', '当たりの最大待機'],
    ['이 시간 동안 입질이 없으면 Exit 후 다시 던짐 (초)', 'If no bite within this time, presses Exit and casts again (sec)', 'この時間内に当たりがなければExit後に再キャスト（秒）'],
    ['미리 누르기', 'Click ahead', '先押し'],
    ['떨어지는 속도를 보고 이만큼 미리 누름', 'Clicks this much earlier based on the falling speed', '落ちる速さを見てこの分だけ先に押す'],
    ['구간을 자꾸 넘어가면 늘리고, 못 따라가면 줄임 (ms)', 'raise it if it keeps overshooting the zone, lower it if it lags (ms)', '区間を何度も越えるなら増やし、追いつかないなら減らす（ms）'],
    ['클릭 최소 간격', 'Min click interval', '最小クリック間隔'],
    ['릴링 중 클릭 사이 최소 간격 (ms)', 'Minimum time between clicks while reeling (ms)', 'リール中のクリック間の最小間隔（ms）'],
    ['결과창 대기', 'Result window wait', '結果ウィンドウの待機'],
    ['릴링이 끝난 뒤 결과창 X 를 누르기까지 (초)', 'From the end of reeling until clicking the result X (sec)', 'リール終了から結果のXを押すまで（秒）'],
    ['Fish 다시 누르기', 'Fish retries', 'Fishの再押し'],
    ['Fish 를 눌러도 반응이 없으면 다시 누르는 횟수', 'How many times to press Fish again when it does not respond', 'Fishを押しても反応がない時に押し直す回数'],
    ['넘으면 인벤토리 가득으로 봄', 'beyond that, the inventory counts as full', '超えるとインベントリ満杯とみなす'],
    ['입질 기다리는 중', 'Waiting for a bite', '当たり待ち'],
    ['릴링 중', 'Reeling', 'リール中'],
    ['결과창 닫기', 'Closing result window', '結果ウィンドウを閉じる'],
    ['낚시 화면 확인 중', 'Checking fishing screen', '釣り画面を確認中'],
    ['낚시 버튼이 안 보임 — 대기', 'Fishing button not visible — waiting', '釣りボタンが見えない — 待機'],
    ['인벤토리 가득 — 판매 필요', 'Inventory full — needs selling', 'インベントリ満杯 — 売却が必要'],
    ['다른 기능에 자리 양보 중', 'Yielding to another feature', '他の機能に譲っている'],
    ['낚시 중', 'Fishing', '釣り中'],
    ['Fish 클릭', 'Clicking Fish', 'Fishをクリック'],
    ['자동 낚시 시작', 'Auto fishing started', '自動釣り開始'],
    ['자동 낚시 정지', 'Auto fishing stopped', '自動釣り停止'],
    ['F7', 'F7', 'F7'],
    ['매크로 꺼짐', 'Macro off', 'マクロ オフ'],
    ['지정된 위치 없음', 'No positions set', '位置が未指定'],
    ['Fish (파랑)', 'Fish (blue)', 'Fish（青）'],
    ['Exit (빨강)', 'Exit (red)', 'Exit（赤）'],
    ['안 보임', 'Not visible', '見えない'],
    ['성공', 'Success', '成功'],
    ['쓰레기', 'Junk', 'ゴミ'],
    ['실패', 'Fail', '失敗'],
    ['결과 확인 안 함', 'Result not checked', '結果未確認'],
    ['릴링 바 영역 저장', 'Reel bar area saved', 'リールバーの範囲を保存'],
    // Acrux 탭
    ['Acrux 설정', 'Acrux settings', 'Acrux設定'],
    ['OCR 감지 방식', 'OCR detection', 'OCR検知方式'],
    ['언어', 'Language', '言語'],
    ['데이터 폴더', 'Data folder', 'データフォルダー'],
    ['화면 글자 읽는 엔진', 'Engine that reads on-screen text', '画面の文字を読むエンジン'],
    ['일반', 'General', '一般'],
    ['OCR 엔진', 'OCR engine', 'OCRエンジン'],
    ['오토 팝핑 · 레어 바이옴 자동 팝핑에서 아이템 이름 · 개수를 읽을 때 씀', 'Used to read item names · counts in auto popping · rare biome auto popping', 'オートポッピング・レアバイオーム自動ポッピングでアイテム名・個数を読む時に使用'],
    ['자동 (추천)', 'Auto (recommended)', '自動（おすすめ）'],
    ['지금 쓰는 엔진', 'Engine in use', '使用中のエンジン'],
    ['RapidOCR 를 골랐어도 설치가 안 돼 있으면 윈도우 OCR 을 씀', 'Falls back to Windows OCR if RapidOCR is not installed', 'RapidOCRを選んでも未インストールならWindows OCRを使用'],
    ['<b>자동</b>: RapidOCR 가 있으면 RapidOCR, 없으면 윈도우 OCR', '<b>Auto</b>: RapidOCR if available, otherwise Windows OCR', '<b>自動</b>: RapidOCRがあればRapidOCR、なければWindows OCR'],
    ['<b>RapidOCR</b>: 게임 글자(테두리 · 배경색)에 강하고 더 정확함 · 처음 한 번 불러올 때 조금 느림', '<b>RapidOCR</b>: Handles game text (outlines · backgrounds) well and is more accurate · a bit slow to load the first time', '<b>RapidOCR</b>: ゲーム文字（縁取り・背景色）に強く、より正確 · 初回の読み込みが少し遅い'],
    ['<b>윈도우 OCR</b>: 윈도우 기본 기능 · 가볍지만 덜 정확함 · 윈도우 언어팩(한국어/영어) 필요', '<b>Windows OCR</b>: Built into Windows · light but less accurate · needs a Windows language pack (Korean/English)', '<b>Windows OCR</b>: Windows標準機能 · 軽いが精度は低め · Windowsの言語パック（韓国語/英語）が必要'],
    ['화면 왼쪽 위 Language 와 같은 설정', 'Same as Language at the top left', '画面左上のLanguageと同じ設定'],
    ['설정 · 기록 · 템플릿이 저장되는 곳', 'Where settings · logs · templates are saved', '設定・記録・テンプレートの保存場所'],
    ['폴더 열기', 'Open folder', 'フォルダーを開く'],
    ['버전', 'Version', 'バージョン'],
    ['확인 중…', 'Checking…', '確認中…'],
    ['폴더를 열 수 없음', 'Cannot open the folder', 'フォルダーを開けません'],
    // 메인 화면 기능별 버튼
    ['오토 스나이핑', 'Auto sniping', 'オートスナイプ'],
    ['끄기', 'Off', 'オフ'],
    ['감시 중', 'Watching', '監視中'],
    ['바이옴 확인 중', 'Checking biome', 'バイオーム確認中'],
    ['플레이어 이름 필요', 'Player name needed', 'プレイヤー名が必要'],
    ['스나이핑 중이라 대기', 'Waiting (sniping)', 'スナイプ中のため待機'],
    ['켜진 기능 없음', 'No features on', 'オンの機能なし'],
    ['포션 사용 중', 'Using potions', 'ポーション使用中'],
    ['모두 꺼짐', 'All off', 'すべてオフ'],
    ['아래 버튼으로 켜기', 'turn on with the buttons below', '下のボタンでオン'],
    ['매크로 켜짐', 'Macro on', 'マクロ オン'],
    ['매크로 꺼짐', 'Macro off', 'マクロ オフ'],
    // 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버)
    ['지금 켜져 있는 로블록스(내 서버)에서 레어 바이옴이 시작되면 바로 포션 사용', 'Uses potions right away when a rare biome starts in the Roblox that is open now (your server)', '今開いているRoblox（自分のサーバー）でレアバイオームが始まるとすぐポーションを使用'],
    ['스나이핑으로 들어간 서버에선 안 함', 'Not in servers joined by sniping', 'スナイプで入ったサーバーでは動かない'],
    ['정지: F7', 'Stop: F7', '停止: F7'],
    ['켜기', 'On', 'オン'],
    ['켜져 있는 동안 레어 바이옴이 감지되면 아래 포션 목록대로 사용 (시작 버튼과 무관)', 'While on, uses the potion lists below when a rare biome is detected (independent of the Start button)', 'オンの間、レアバイオームを検知すると下のポーションリスト通りに使用（開始ボタンとは無関係）'],
    ['바이옴 감지에 필요', 'Needed for biome detection', 'バイオーム検知に必要'],
    ['바이옴 매크로 설정 → 기본 설정에서 입력', 'Enter it in Biome macro settings → Basic settings', 'バイオームマクロ設定 → 基本設定で入力'],
    ['입력 안 됨 — 바이옴 감지 안 됨', 'Not set — biomes not detected', '未入力 — バイオームを検知しない'],
    ['버튼 위치 · OCR · 딜레이', 'Button positions · OCR · Delays', 'ボタン位置・OCR・ディレイ'],
    ['오토 팝핑 매크로 설정(스나이프 탭)에 지정한 것을 같이 씀', 'Uses the ones set in Auto popping macro settings (Snipe tab)', 'オートポッピングマクロ設定（スナイプタブ）で指定したものを共用'],
    ['지정됨', 'Set', '指定済み'],
    ['시작 전 대기', 'Wait before start', '開始前の待機'],
    ['바이옴 감지 후 인벤토리를 열기까지 (초)', 'From biome detection until opening the inventory (sec)', 'バイオーム検知からインベントリを開くまで（秒）'],
    ['기본 1', 'Default 1', 'デフォルト 1'],
    ['다 쓰고 인벤토리 닫기', 'Close inventory when done', '使い終わったらインベントリを閉じる'],
    ['Inventory 버튼을 한 번 더 눌러 닫음', 'Clicks the Inventory button once more to close it', 'Inventoryボタンをもう一度押して閉じる'],
    ['켜져 있어야 이 바이옴에서 팝핑', 'Pops in this biome only when on', 'オンの時のみこのバイオームでポッピング'],
    ['포션 목록은 아래', 'Potion list below', 'ポーションリストは下'],
    ['레어 바이옴 자동 팝핑 켜짐', 'Rare biome auto popping on', 'レアバイオーム自動ポッピング オン'],
    ['레어 바이옴 자동 팝핑 꺼짐', 'Rare biome auto popping off', 'レアバイオーム自動ポッピング オフ'],
    ['레어 바이옴 자동 팝핑 테스트 시작', 'Rare biome auto popping test started', 'レアバイオーム自動ポッピングのテスト開始'],
    ['인벤토리 열기부터', 'from opening the inventory', 'インベントリを開くところから'],
    ['정지: 위쪽 [정지] 또는 F7', 'Stop: [Stop] above or F7', '停止: 上の[停止]またはF7'],
    ['포션 목록이 비어 있음', 'Potion list is empty', 'ポーションリストが空です'],
    ['레어 바이옴 자동 팝핑 정지', 'Rare biome auto popping stopped', 'レアバイオーム自動ポッピング停止'],
    ['Inventory 닫기', 'Closing Inventory', 'Inventoryを閉じる'],
    ['카드 짝 맞추기', 'Card pair matching', 'カードのペア合わせ'],
    ['자동 낚시', 'Auto fishing', '自動釣り'],
    ['던지기 · 릴링 · 판매', 'Cast · Reel · Sell', 'キャスト・リール・売却'],
    ['기준 위치', 'Base position', '基準位置'],
    ['모든 기능의 출발점', 'Starting point for every feature', '全機能の出発点'],
    ['이번 실행', 'This run', '今回の実行'],
    ['매크로를 켠 뒤부터', 'Since the macro started', 'マクロ開始から'],
    ['올타임', 'All-time', 'オールタイム'],
    ['지금까지 전부', 'Everything so far', 'これまでの全部'],
    ['아직 기록 없음', 'No records yet', 'まだ記録なし'],
    ['이전 탭', 'Previous tab', '前のタブ'],
    ['다음 탭', 'Next tab', '次のタブ'],
    ['로그', 'Log', 'ログ'],
    ['동작 기록', 'Activity log', '動作ログ'],
    ['후원하기', 'Support me', '支援する'],
    ['크레딧', 'Credits', 'クレジット'],
    ['만든 사람 · 링크', 'Creator · Links', '制作者・リンク'],
    ['바이옴 매크로 켜기/끄기', 'Turn biome macro on/off', 'バイオームマクロのオン/オフ'],
    ['디스코드 서버 ›', 'Discord server ›', 'Discordサーバー ›'],
    ['로블록스 프로필 ›', 'Roblox profile ›', 'Robloxプロフィール ›'],
    ['설정 저장 실패', 'Failed to save settings', '設定の保存に失敗'],
    ['감시 대상 없음', 'No watch targets', '監視対象なし'],
    ['감지만 (링크 안 엶)', 'Detect only (links not opened)', '検知のみ（リンクは開かない）'],
    ['작동 중', 'Running', '作動中'],
    ['대기 — 시작 시 작동', 'Idle — runs after Start', '待機 — 開始で作動'],
    ['감시 꺼짐', 'Watching off', '監視オフ'],
    ['프로그램과 연결 끊김', 'Lost connection to the program', 'プログラムとの接続が切れました'],

    // 디스코드 감지 설정
    ['감시 켜짐', 'Watching', '監視オン'],
    ['실제 접속', 'Auto join', '自動参加'],
    ['감지 기록', 'Detection log', '検知履歴'],
    ['최근에 잡힌 링크', 'Recently caught links', '最近検知したリンク'],
    ['감시 대상', 'Watch targets', '監視対象'],
    ['서버 · 채널 목록', 'Servers · Channels', 'サーバー・チャンネル一覧'],
    ['필터', 'Filter', 'フィルター'],
    ['바이옴 설정', 'Biome settings', 'バイオーム設定'],
    ['기타 설정', 'Other settings', 'その他の設定'],
    ['디스코드 · 알림', 'Discord · Alerts', 'Discord・通知'],
    ['지우기', 'Clear', 'クリア'],
    ['시간', 'Time', '時間'],
    ['상태', 'Status', '状態'],
    ['서버 #채널', 'Server #Channel', 'サーバー #チャンネル'],
    ['링크', 'Link', 'リンク'],
    ['감지된 링크 없음', 'No links detected', '検知したリンクはありません'],
    ['등록한 서버 / 채널만 감지', 'Only registered servers / channels are watched', '登録したサーバー / チャンネルのみ検知'],
    ['비어 있으면 감지 안 함', 'Nothing is detected if empty', '空の場合は検知しません'],
    ['튜토리얼 보기', 'View tutorial', 'チュートリアルを見る'],
    ['스나이핑 안정성 설정', 'Snipe stability', 'スナイプ安定性設定'],
    ['사람처럼 접속 · 안티 스나이핑 대비', 'Human-like joins · anti-snipe ready', '人間らしい参加 · アンチスナイプ対策'],
    ['접속', 'Join', '参加'],
    ['마우스', 'Mouse', 'マウス'],
    ['딜레이 · 링크 열기 방식', 'Delay · link open mode', '遅延 · リンクの開き方'],
    ['사람처럼 접속', 'Human-like joins', '人間らしい参加'],
    ['사람처럼 클릭', 'Human-like clicks', '人間らしいクリック'],
    ['링크 감지 후 접속 딜레이', 'Join delay after detecting a link', 'リンク検知後の参加遅延'],
    ['링크를 보고 직접 누르는 것처럼, 최소 ~ 최대 사이에서 매번 다르게 기다린 뒤 접속 (초) · 기본 2 ~ 4', 'Like seeing the link and clicking it yourself, waits a different time between min ~ max each time before joining (s) · default 2 ~ 4', 'リンクを見て自分で押すように、最小〜最大の間で毎回違う時間待ってから参加（秒）· 既定 2〜4'],
    ['로블록스 바로 실행', 'Launch Roblox directly', 'Robloxを直接起動'],
    ['켜짐: 로블록스를 바로 실행 (가장 빠름) · 꺼짐: 웹브라우저로 링크를 열어서 접속 (사람이 링크를 누른 것과 같은 방법)', 'On: launch Roblox directly (fastest) · Off: open the link in your web browser and join (the same way as a person clicking it)', 'オン: Robloxを直接起動（最速）· オフ: Webブラウザでリンクを開いて参加（人がリンクを押すのと同じ方法）'],
    ['같은 서버 다시 안 들어가기', 'Don\'t rejoin the same server', '同じサーバーに再参加しない'],
    ['같은 서버 링크는 이 시간 동안 한 번만 접속 (분) · 0 = 끔 · 기본 10', 'Joins the same server link only once within this time (min) · 0 = off · default 10', '同じサーバーのリンクはこの時間内に1回だけ参加（分）· 0 = オフ · 既定 10'],
    ['연속 접속 최소 간격', 'Minimum time between joins', '連続参加の最小間隔'],
    ['한 번 접속한 뒤 이 시간 동안은 다른 링크를 타지 않음 (초) · 기본 5', 'After joining, won\'t take another link for this long (s) · default 5', '一度参加したらこの時間は他のリンクに乗らない（秒）· 既定 5'],
    ['마우스 사람처럼 움직이기', 'Move the mouse like a person', 'マウスを人のように動かす'],
    ['순간이동하지 않고 부드럽게 이동 · 누르는 시간도 매번 조금씩 다르게', 'Moves smoothly instead of teleporting · press time varies a little each time', '瞬間移動せず滑らかに移動 · 押す時間も毎回少しずつ変える'],
    ['클릭 위치 랜덤', 'Random click position', 'クリック位置ランダム'],
    ['지정한 위치에서 이 범위 안으로 살짝 다르게 클릭 (px) · 0 = 끔 · 기본 3', 'Clicks slightly differently within this range of the set position (px) · 0 = off · default 3', '指定位置からこの範囲内で少しずらしてクリック（px）· 0 = オフ · 既定 3'],
    ['Play 버튼 · 오토 팝핑 · 복귀 후 동작의 클릭에 모두 적용됩니다.', 'Applies to all clicks in the Play button, auto popping and after-return actions.', 'Playボタン · オートポッピング · 復帰後の動作のクリックすべてに適用されます。'],
    ['중복', 'Duplicate', '重複'],
    ['건너뜀', 'Skipped', 'スキップ'],
    ['접속 취소 — 작동 중지됨', 'Join canceled — stopped', '参加キャンセル — 停止中'],
    ['링크 열기 방식', 'Link open mode', 'リンクの開き方'],
    ['중복 · 연속 접속 막기', 'Prevent duplicate & back-to-back joins', '重複 · 連続参加の防止'],
    ['일부 서버에는 <b>안티 스나이핑</b>(링크가 올라오자마자 들어오는 사람을 막는 장치)이 있습니다.', 'Some servers have <b>anti-sniping</b> (something that blocks people who join the moment a link is posted).', '一部のサーバーには<b>アンチスナイプ</b>（リンクが投稿された瞬間に入る人を防ぐ仕組み）があります。'],
    ['이 설정으로 링크를 보고 <b>사람이 직접 누른 것처럼</b> 잠깐 기다렸다가 접속하게 할 수 있습니다.', 'These settings make the macro wait a moment <b>as if a person saw and clicked the link</b> before joining.', 'この設定で、リンクを見て<b>人が自分で押したように</b>少し待ってから参加させられます。'],
    ['이 튜토리얼은 건너뛸 수 없습니다.', 'This tutorial can\'t be skipped.', 'このチュートリアルはスキップできません。'],
    ['스나이프 탭의 강조된 <b>스나이핑 안정성 설정</b> 버튼을 직접 눌러주세요.', 'Click the highlighted <b>Snipe stability</b> button in the Snipe tab yourself.', 'スナイプタブの強調された<b>スナイプ安定性設定</b>ボタンを直接クリックしてください。'],
    ['링크를 감지한 뒤 접속하기 전까지 <b>최소 ~ 최대 사이에서 매번 다르게</b> 기다립니다.', 'After detecting a link, waits <b>a different time between min ~ max each time</b> before joining.', 'リンクを検知してから参加するまで、<b>最小〜最大の間で毎回違う時間</b>待ちます。'],
    ['기본 2 ~ 4초. 둘 다 0 이면 바로 접속합니다. 기다리는 중에 매크로를 중지하면 접속하지 않습니다.', 'Default 2 ~ 4s. If both are 0 it joins right away. If you stop the macro while waiting, it won\'t join.', '既定 2〜4秒。両方 0 ならすぐ参加します。待っている間にマクロを停止すると参加しません。'],
    ['<b>켜짐</b>: 지금처럼 로블록스를 바로 실행합니다 (가장 빠름).', '<b>On</b>: launches Roblox directly, as now (fastest).', '<b>オン</b>: 今まで通りRobloxを直接起動します（最速）。'],
    ['<b>꺼짐</b>: 기본 웹브라우저로 링크를 열어서, 사람이 링크를 누른 것과 같은 방법으로 접속합니다.', '<b>Off</b>: opens the link in your default web browser and joins the same way a person clicking the link would.', '<b>オフ</b>: 既定のWebブラウザでリンクを開き、人がリンクを押すのと同じ方法で参加します。'],
    ['꺼짐으로 쓸 때는 브라우저에서 처음 한 번 Roblox 열기를 누르면서 <b>항상 허용</b>에 체크해주세요. 그래야 다음부터 자동으로 열립니다.', 'When using Off, the first time the browser asks to open Roblox, check <b>Always allow</b>. Then it opens automatically from then on.', 'オフで使う場合、ブラウザで初めてRobloxを開くときに<b>常に許可</b>にチェックしてください。次回から自動で開きます。'],
    ['<b>같은 서버 다시 안 들어가기</b>: 같은 서버 링크가 여러 채널에 올라와도 정한 시간(분) 동안 한 번만 들어갑니다.', '<b>Don\'t rejoin the same server</b>: even if the same server link is posted in several channels, it joins only once within the set time (min).', '<b>同じサーバーに再参加しない</b>: 同じサーバーのリンクが複数のチャンネルに投稿されても、決めた時間（分）内に1回だけ参加します。'],
    ['<b>연속 접속 최소 간격</b>: 한 번 접속한 뒤 정한 시간(초) 동안은 다른 링크를 타지 않습니다.', '<b>Minimum time between joins</b>: after joining, it won\'t take another link for the set time (s).', '<b>連続参加の最小間隔</b>: 一度参加したら決めた時間（秒）は他のリンクに乗りません。'],
    ['접속을 기다리는 중에 올라온 다른 링크도 건너뜁니다.', 'Other links posted while it waits to join are skipped too.', '参加待ちの間に投稿された他のリンクもスキップします。'],
    ['<b>마우스 사람처럼 움직이기</b>: 클릭할 곳으로 순간이동하지 않고 부드럽게 움직이며, 누르는 시간도 매번 조금씩 다릅니다.', '<b>Move the mouse like a person</b>: moves smoothly to where it clicks instead of teleporting, and the press time varies a little each time.', '<b>マウスを人のように動かす</b>: クリック先へ瞬間移動せず滑らかに動き、押す時間も毎回少しずつ変わります。'],
    ['<b>클릭 위치 랜덤</b>: 지정한 위치에서 이 범위(px) 안으로 살짝 다르게 누릅니다.', '<b>Random click position</b>: clicks slightly differently within this range (px) of the set position.', '<b>クリック位置ランダム</b>: 指定位置からこの範囲（px）内で少しずらして押します。'],
    ['Play 버튼 · 오토 팝핑 · 복귀 후 동작의 클릭에 모두 적용됩니다. 누를 버튼이 작으면 범위를 줄여주세요.', 'Applies to all clicks in the Play button, auto popping and after-return actions. If the buttons are small, lower the range.', 'Playボタン · オートポッピング · 復帰後の動作のクリックすべてに適用されます。押すボタンが小さい場合は範囲を小さくしてください。'],
    ['스나이핑 안정성 설정이 끝났습니다.', 'Snipe stability setup is done.', 'スナイプ安定性設定が完了しました。'],
    ['언제든 스나이프 탭의 <b>스나이핑 안정성 설정</b>에서 바꿀 수 있습니다.', 'You can change it anytime from <b>Snipe stability</b> in the Snipe tab.', 'スナイプタブの<b>スナイプ安定性設定</b>からいつでも変更できます。'],
    ['서버', 'Server', 'サーバー'],
    ['이 서버의 모든 채널', 'All channels in this server', 'このサーバーの全チャンネル'],
    ['채널', 'Channel', 'チャンネル'],
    ['이 채널만 (스레드 제외)', 'This channel only (no threads)', 'このチャンネルのみ（スレッド除く）'],
    ['서버 ID', 'Server ID', 'サーバーID'],
    ['채널 ID', 'Channel ID', 'チャンネルID'],
    ['추가', 'Add', '追加'],
    ['선택한 바이옴이 포함된 링크만 감지', 'Only links mentioning a selected biome', '選択したバイオームを含むリンクのみ検知'],
    ['선택 없으면 전부 감지', 'Detects everything if none selected', '未選択ならすべて検知'],
    ['바이옴 이름 직접 추가', 'Add a biome name', 'バイオーム名を追加'],
    ['자동 제외', 'Auto exclude', '自動除外'],
    ['<b>Ended</b>, <b>End</b> 단어가 포함된 메시지는 무시 (대소문자 구분 X, 단어 단위)',
     'Messages containing the word <b>Ended</b> or <b>End</b> are ignored (case-insensitive, whole words)',
     '<b>Ended</b>、<b>End</b> を含むメッセージは無視（大文字・小文字の区別なし、単語単位）'],
    ['디스코드 종류', 'Discord version', 'Discordの種類'],
    ['디버그 포트', 'Debug port', 'デバッグポート'],
    ['자동 재시작', 'Auto restart', '自動再起動'],
    ['디코가 일반 모드로 켜져 있으면 재시작', 'Restarts Discord if it was opened normally', 'Discordが通常モードで起動中なら再起動'],
    ['바로 작동', 'Run on launch', '起動時に開始'],
    ['프로그램 켤 때 시작 버튼까지 눌린 상태로', 'Starts with Start already pressed', '起動時に開始ボタンが押された状態にする'],
    ['소리', 'Sound', 'サウンド'],
    ['감지되면 알림음', 'Beep when a link is detected', '検知時に通知音'],
    ['안정화 접속', 'Stable join', '安定参加'],
    ['로블록스 전부 종료 → 0.5초 후 실행', 'Close all Roblox → launch after 0.5s', 'Robloxをすべて終了 → 0.5秒後に起動'],
    ['끄면 즉시 실행', 'Launches instantly when off', 'オフなら即起動'],
    ['모든 링크 표시', 'Show all links', 'すべてのリンクを表示'],
    ["감시 대상 밖 링크도 기록에 '보임'으로", "Links outside your targets are logged as 'Seen'", '監視対象外のリンクも「表示」として記録'],
    ['프로그램 폴더 열기', 'Open program folder', 'プログラムフォルダを開く'],
    ['삭제', 'Delete', '削除'],
    ['알 수 없음', 'Unknown', '不明'],
    ['비어 있음', 'Empty', '空'],
    ['이름 모름', 'Name unknown', '名前不明'],
    ['디스코드 연결 시 가져옴', 'Fetched when Discord connects', 'Discord接続時に取得'],
    ['복사', 'Copy', 'コピー'],
    ['ID 복사됨', 'ID copied', 'IDをコピーしました'],
    ['복사 실패', 'Copy failed', 'コピー失敗'],
    ['서버 추가됨', 'Server added', 'サーバーを追加しました'],
    ['채널 추가됨', 'Channel added', 'チャンネルを追加しました'],
    ['숫자 ID만 입력 가능', 'Numbers only', '数字のIDのみ入力できます'],
    ['추가됨', 'Added', '追加しました'],
    ['디스코드 연결 후 이름을 가져옴', 'Name is fetched after Discord connects', 'Discord接続後に名前を取得'],
    ['열림', 'Opened', 'オープン'],
    ['보임', 'Seen', '表示'],
    ['바이옴 불일치', 'Biome mismatch', 'バイオーム不一致'],
    ['제외됨', 'Excluded', '除外'],
    ['꺼짐', 'Off', 'オフ'],
    ['켜짐', 'On', 'オン'],
    ['열기', 'Open', '開く'],
    ['링크 복사됨', 'Link copied', 'リンクをコピーしました'],

    // 바이옴 매크로 설정
    ['바이옴 인식 안 됨', 'No biome detected', 'バイオーム未検出'],
    ['바이옴 매크로', 'Biome macro', 'バイオームマクロ'],
    ['기본 설정', 'Basic settings', '基本設定'],
    ['플레이어 이름 · 브섭 링크', 'Player name · PS link', 'プレイヤー名・PSリンク'],
    ['디스코드 웹후크', 'Discord webhook', 'Discord Webhook'],
    ['알림 받을 주소', 'Where alerts go', '通知の送信先'],
    ['멘션 설정', 'Mentions', 'メンション設定'],
    ['바이옴별 역할 · @everyone', 'Roles per biome · @everyone', 'バイオーム別ロール・@everyone'],
    ['최근 바이옴', 'Recent biomes', '最近のバイオーム'],
    ['인식된 바이옴 기록', 'Detected biome history', '検出したバイオームの履歴'],
    ['플레이어 이름', 'Player name', 'プレイヤー名'],
    ['디스플레이 이름이 아닌 실제 닉네임 (@ 뒤 이름)', 'Your username, not display name (the name after @)', '表示名ではなくユーザー名（@の後の名前）'],
    ['게임 속 닉네임과 같아야 바이옴 감지', 'Must match your in-game username', 'ゲーム内のユーザー名と一致する必要あり'],
    ['브섭 링크', 'PS link', 'PSリンク'],
    ['바이옴 시작 알림에 함께 표시', 'Shown in biome start alerts', 'バイオーム開始通知に表示'],
    ['바이옴 인식 상태', 'Biome detection', 'バイオーム検出状態'],
    ['로블록스 로그 파일로 인식', 'Read from Roblox log files', 'Robloxのログファイルで検出'],
    ['화면 인식(OCR) 불필요', 'No screen reading (OCR) needed', '画面認識（OCR）不要'],
    ['바이옴 매크로 토글이 켜져 있을 때만 알림 전송 (시작 버튼과 무관)', 'Alerts are sent only while the Biome macro toggle is on (separate from Start)', 'バイオームマクロのトグルがオンの時のみ通知（開始ボタンとは無関係）'],
    ['프로그램을 켜면 항상 꺼진 상태로 시작', 'Always off when the program starts', '起動時は常にオフ'],
    ['바이옴이 시작될 때 함께 멘션', 'Mentioned when a biome starts', 'バイオーム開始時にメンション'],
    ['역할 ID 는 디스코드에서 역할 우클릭 → ID 복사하기 (개발자 모드 필요)', 'Get a role ID by right-clicking the role in Discord → Copy ID (Developer Mode required)', 'ロールIDはDiscordでロールを右クリック → IDをコピー（開発者モードが必要）'],
    ['기록 없음', 'No history', '履歴なし'],
    ['테스트 전송', 'Send test', 'テスト送信'],
    ['선택', 'Optional', '任意'],
    ['보내는 알림', 'Alerts sent', '送信される通知'],
    ['<b>Biome Macro Enabled / Disabled</b> — 바이옴 매크로가 켜지거나 꺼질 때 (Acrux 다운로드 링크 포함)',
     '<b>Biome Macro Enabled / Disabled</b> — when the biome macro is turned on or off (with Acrux download link)',
     '<b>Biome Macro Enabled / Disabled</b> — バイオームマクロのオン/オフ時（Acruxダウンロードリンク付き）'],
    ['<b>Biome Started - 바이옴</b> — 시작 시각, Uptime, 브섭 링크, 바이옴 썸네일 + 멘션 설정의 역할 / @everyone',
     '<b>Biome Started - Biome</b> — start time, uptime, PS link, biome thumbnail + roles / @everyone from Mentions',
     '<b>Biome Started - バイオーム</b> — 開始時刻、Uptime、PSリンク、バイオームのサムネイル + メンション設定のロール / @everyone'],
    ['<b>Biome Ended - 바이옴</b> — 바이옴이 끝났을 때',
     '<b>Biome Ended - Biome</b> — when the biome ends',
     '<b>Biome Ended - バイオーム</b> — バイオーム終了時'],
    ['<b>Player has left the game</b> — 로블록스 접속이 끊겼을 때',
     '<b>Player has left the game</b> — when you disconnect from Roblox',
     '<b>Player has left the game</b> — Robloxから切断された時'],
    ['플레이어 이름을 먼저 입력해주세요', 'Enter your player name first', '先にプレイヤー名を入力してください'],
    ['바이옴 매크로 켜짐', 'Biome macro on', 'バイオームマクロ オン'],
    ['바이옴 매크로 꺼짐', 'Biome macro off', 'バイオームマクロ オフ'],
    ['역할 ID (선택)', 'Role ID (optional)', 'ロールID（任意）'],
    ['전송 중…', 'Sending…', '送信中…'],
    ['플레이어 이름 입력 필요', 'Player name required', 'プレイヤー名の入力が必要'],
    ['이름이 없으면 감지 안 함', 'Nothing is detected without it', '名前がないと検出しません'],
    ['로블록스 로그 파일 없음', 'No Roblox log file', 'Robloxのログファイルなし'],
    ['로블록스를 한 번 실행하면 생김', 'Created after Roblox runs once', 'Robloxを一度起動すると作成されます'],
    ['닉네임 불일치', 'Username mismatch', 'ユーザー名の不一致'],
    ['게임 속 실제 닉네임과 입력한 이름이 다름', 'The name differs from your in-game username', '入力した名前がゲーム内のユーザー名と違います'],
    ['계정 확인 중', 'Checking account', 'アカウント確認中'],
    ['게임에 들어가면 닉네임 확인', 'Checked once you join a game', 'ゲームに入るとユーザー名を確認'],
    ['인식 중', 'Detecting', '検出中'],
    ['로그 확인 중', 'Checking logs', 'ログ確認中'],
    ['게임에 들어가면 인식', 'Detected once you join a game', 'ゲームに入ると検出'],

    // 오토 팝핑 매크로 설정
    ['대기', 'Idle', '待機'],
    ['테스트', 'Test', 'テスト'],
    ['정지', 'Stop', '停止'],
    ['접속 전 동작', 'Before join', '参加前の動作'],
    ['프로그램 종료 · 키 입력', 'Close programs · Key press', 'プログラム終了・キー入力'],
    ['게임 접속', 'Game join', 'ゲーム参加'],
    ['Play 버튼 위치 · 대기', 'Play button position · Wait', 'Playボタン位置・待機'],
    ['오토 팝핑 설정', 'Auto popping settings', 'オートポッピング設定'],
    ['버튼 위치 · OCR', 'Button positions · OCR', 'ボタン位置・OCR'],
    ['팝핑 바이옴 설정', 'Popping biomes', 'ポッピング対象バイオーム'],
    ['팝핑할 바이옴 켜기 · 끄기', 'Turn biomes on · off', 'バイオームのオン・オフ'],
    ['오토 팝핑 딜레이 설정', 'Auto popping delays', 'オートポッピングの待ち時間'],
    ['동작 사이 대기', 'Wait between actions', '動作間の待機'],
    ['포션 템플릿', 'Potion template', 'ポーションテンプレート'],
    ['링크로 게임에 접속하기 직전에 순서대로 실행', 'Runs in order right before joining from a link', 'リンクから参加する直前に順番に実行'],
    ['끝나면 접속', 'then joins', '終わったら参加'],
    ['테스트 실행', 'Test run', 'テスト実行'],
    ['동작 없음', 'No actions', '動作なし'],
    ['필요하면 위 버튼으로 추가 (예: 특정 프로그램 강제 종료, 키 입력)', 'Add one with the buttons above if needed (e.g. force-close a program, press a key)', '必要なら上のボタンで追加（例: 特定プログラムの強制終了、キー入力）'],
    ['직접 지정 필요', 'Must be set manually', '手動で指定が必要'],
    ['로블록스 창 기준이라 창 크기가 바뀌어도 그대로', 'Relative to the Roblox window, so resizing is fine', 'Robloxウィンドウ基準なのでサイズが変わってもそのまま'],
    ['위치 지정', 'Set position', '位置を指定'],
    ['Play 버튼과 번갈아 클릭', 'Clicked alternately with Play', 'Playボタンと交互にクリック'],
    ['클릭 간격', 'Click interval', 'クリック間隔'],
    ['Play ↔ Click to skip 번갈아 누르는 간격 (초)', 'Gap between alternating Play ↔ Click to skip clicks (s)', 'Play ↔ Click to skip を交互に押す間隔（秒）'],
    ['접속 후 대기', 'Wait after launch', '起動後の待機'],
    ['로블록스 창이 뜬 뒤 첫 클릭까지 (초)', 'From the Roblox window opening to the first click (s)', 'Robloxウィンドウ表示から最初のクリックまで（秒）'],
    ['최대 시도 시간', 'Max try time', '最大試行時間'],
    ['이 시간 안에 입장이 안 되면 중지 (초)', "Stops if you haven't entered by then (s)", 'この時間内に入れなければ停止（秒）'],
    ['기본 템플릿으로', 'Reset to default', 'デフォルトに戻す'],
    ['이 템플릿 테스트', 'Test this template', 'このテンプレートをテスト'],
    ['1초 후 Play / Click to skip 번갈아 클릭 시작', 'Clicking Play / Click to skip alternately in 1s', '1秒後にPlay / Click to skipを交互にクリック開始'],
    ['게임 입장이 확인되면 자동 정지', "Stops automatically once you're in game", 'ゲーム参加を確認すると自動停止'],
    ['접속 중… (로블록스가 뜨면 사라짐)', 'Joining… (disappears when Roblox opens)', '参加中…（Robloxが開くと消えます）'],
    ['내 서버로 접속', 'Joining your server', '自分のサーバーに参加'],
    ['오토 팝핑', 'Auto popping', 'オートポッピング'],
    ['로블록스 창 감지됨', 'Roblox window found', 'Robloxウィンドウ検出'],
    ['로블록스 창 없음', 'No Roblox window', 'Robloxウィンドウなし'],
    ['실행 중', 'Running', '実行中'],
    ['지정 안 됨', 'Not set', '未設定'],
    ['위치 저장', 'Position saved', '位置を保存'],
    ['아이템 칸', 'Item slot', 'アイテム欄'],
    ['검색 결과 첫 칸', 'First search result', '検索結果の最初の欄'],
    ['수량 입력칸', 'Amount box', '数量入力欄'],
    ['OCR 영역', 'OCR area', 'OCR範囲'],
    ['검색 결과 아이템 이름·개수 (예: Warp Potion x23)', 'Item name·count in the search result (e.g. Warp Potion x23)', '検索結果のアイテム名・個数（例: Warp Potion x23）'],
    ['드래그로 지정', 'Drag to set', 'ドラッグで指定'],
    ['OCR 테스트', 'OCR test', 'OCRテスト'],
    ['이름 일치율 기준', 'Name match threshold', '名前一致率の基準'],
    ['OCR 이름과 포션 이름이 이 이상 같아야 사용', 'Used only if the OCR name matches the potion name at least this much', 'OCRの名前とポーション名がこれ以上一致すれば使用'],
    ['미만이면 1회 재검색 후 스킵 (%)', 'Below it, searches once more then skips (%)', '未満なら1回再検索してスキップ（%）'],
    ['로블록스 화면에서 드래그', 'Drag in Roblox', 'Robloxでドラッグ'],
    ['OCR 영역 저장', 'OCR area saved', 'OCR範囲を保存'],
    ['읽는 중…', 'Reading…', '読み取り中…'],
    ['OCR 응답 없음', 'No OCR response', 'OCRの応答なし'],
    ['로그 확인', 'check the log', 'ログを確認'],
    ['표시 없음(1개)', 'not shown (1)', '表示なし（1個）'],
    ['읽은 글자 없음', 'No text read', '読み取った文字なし'],
    ['(읽은 글자 없음)', '(no text read)', '（読み取った文字なし）'],
    ['Play 후 게임 입장 → 시작', 'After Play, entering game → start', 'Play後のゲーム参加 → 開始'],
    ['아이템 이름 입력 + 엔터 후', 'After typing name + Enter', 'アイテム名入力 + Enter後'],
    ['아이템 클릭 후', 'After clicking item', 'アイテムクリック後'],
    ['수량칸 더블클릭 간격', 'Amount box double-click gap', '数量欄ダブルクリック間隔'],
    ['수량칸 더블클릭 후', 'After amount box double-click', '数量欄ダブルクリック後'],
    ['개수 입력 + 엔터 후', 'After typing amount + Enter', '個数入力 + Enter後'],
    ['사용 방식', 'Mode', '使用方法'],
    ['목록 위쪽일수록 먼저', 'Higher in the list goes first', 'リストの上ほど先に使用'],
    ['목록이 비어 있으면 이 바이옴에선 오토 팝핑 안 함', 'No auto popping in this biome if the list is empty', 'リストが空ならこのバイオームではオートポッピングしない'],
    ['목록 전부 순서대로', 'Whole list in order', 'リストを全部順番に'],
    ['위에서부터 조건 맞는 하나만', 'Only the first one that qualifies', '上から条件に合う1つだけ'],
    ['+ 포션 추가', '+ Add potion', '+ ポーション追加'],
    ['최소 보유: OCR 로 읽은 개수가 이보다 적으면 그 포션은 건너뜀', 'Min owned: skips the potion if the OCR count is lower', '最低所持数: OCRで読んだ個数がこれより少なければそのポーションはスキップ'],
    ['포션 없음', 'No potions', 'ポーションなし'],
    ['아래 버튼으로 추가', 'Add one with the button below', '下のボタンで追加'],
    ['포션 이름 (예: Warp Potion)', 'Potion name (e.g. Warp Potion)', 'ポーション名（例: Warp Potion）'],
    ['전부', 'All', '全部'],
    ['직접', 'Custom', '指定'],
    ['사용 개수', 'Amount to use', '使用個数'],
    ['최소 보유', 'Min owned', '最低所持数'],
    ['최소', 'Min', '最低'],
    ['위로', 'Up', '上へ'],
    ['아래로', 'Down', '下へ'],
    ['켜져 있어야 이 바이옴에서 오토 팝핑', 'Auto pops in this biome only when on', 'オンの時のみこのバイオームでオートポッピング'],
    ['기본 템플릿으로 되돌림', 'Reset to the default template', 'デフォルトテンプレートに戻しました'],
    ['오토 팝핑 테스트 시작', 'Auto popping test started', 'オートポッピングテスト開始'],
    ['인벤토리 열기부터', 'from opening the inventory', 'インベントリを開くところから'],
    ['정지: 위쪽 [정지] 또는 F7', 'Stop: [Stop] at the top or F7', '停止: 上の［停止］またはF7'],

    // 매크로 복귀 설정
    ['복귀 테스트', 'Test return', '復帰テスト'],
    ['복귀 설정', 'Return settings', '復帰設定'],
    ['내 브섭 링크', 'My PS link', '自分のPSリンク'],
    ['복귀 후 동작', 'After return', '復帰後の動作'],
    ['직접 만드는 매크로', 'Your own macro', '自作マクロ'],
    ['바이옴이 끝나거나 오토 팝핑이 어떤 이유로든 끝나면 → 로블록스 전부 종료 → 1초 → 이 링크로 접속 → Play',
     'When the biome ends or auto popping stops for any reason → close all Roblox → 1s → join this link → Play',
     'バイオーム終了またはオートポッピングが何らかの理由で終わると → Robloxをすべて終了 → 1秒 → このリンクで参加 → Play'],
    ['바이옴 매크로 설정의 브섭 링크 사용', 'Use the PS link from Biome macro settings', 'バイオームマクロ設定のPSリンクを使用'],
    ['바이옴 매크로 설정 → 기본 설정에 입력한 링크를 가져옴', 'Copies the link from Biome macro settings → Basic settings', 'バイオームマクロ設定 → 基本設定のリンクを取得'],
    ['가져오기', 'Import', '取り込む'],
    ['내 서버 입장 후 순서대로 실행', 'Runs in order after entering your server', '自分のサーバーに入った後、順番に実行'],
    ['정지: F7', 'Stop: F7', '停止: F7'],
    ['위 버튼으로 추가', 'Add one with the buttons above', '上のボタンで追加'],
    ['바이옴 매크로 설정에 브섭 링크가 없음', 'No PS link in Biome macro settings', 'バイオームマクロ設定にPSリンクがありません'],
    ['브섭 링크 가져옴', 'PS link imported', 'PSリンクを取り込みました'],
    ['로블록스 종료 → 1초 → 내 서버 접속 → Play', 'Close Roblox → 1s → join your server → Play', 'Roblox終了 → 1秒 → 自分のサーバーに参加 → Play'],
    ['복귀 중', 'Returning', '復帰中'],

    // 동작 편집기
    ['키 입력', 'Key press', 'キー入力'],
    ['키 조합', 'Key combo', 'キーの組み合わせ'],
    ['마우스 클릭', 'Mouse click', 'マウスクリック'],
    ['글자 입력', 'Type text', '文字入力'],
    ['스크롤', 'Scroll', 'スクロール'],
    ['프로그램 실행', 'Run program', 'プログラム実行'],
    ['프로그램 강제 종료', 'Force-close program', 'プログラム強制終了'],
    ['창 맨 앞으로', 'Bring window to front', 'ウィンドウを最前面へ'],
    ['복제', 'Duplicate', '複製'],
    ['로블록스에 보내기', 'Send to Roblox', 'Robloxに送る'],
    ['키', 'Key', 'キー'],
    ['횟수', 'Count', '回数'],
    ['누르는 시간 (ms)', 'Hold (ms)', '押す時間 (ms)'],
    ['간격 (ms)', 'Gap (ms)', '間隔 (ms)'],
    ['키 조합 (+ 로 연결)', 'Key combo (join with +)', 'キーの組み合わせ（+でつなぐ）'],
    ['로블록스 화면에서 클릭', 'Click in Roblox', 'Robloxでクリック'],
    ['위치 (로블록스 창 기준)', 'Position (Roblox window)', '位置（Robloxウィンドウ基準）'],
    ['버튼', 'Button', 'ボタン'],
    ['왼쪽', 'Left', '左'],
    ['오른쪽', 'Right', '右'],
    ['시간 (ms)', 'Time (ms)', '時間 (ms)'],
    ['글자', 'Text', '文字'],
    ['글자 (한 번에 붙여넣기)', 'Text (pasted at once)', '文字（一度に貼り付け）'],
    ['입력할 글자', 'Text to type', '入力する文字'],
    ['글자 간격 (ms)', 'Char gap (ms)', '文字間隔 (ms)'],
    ['입력 후 엔터', 'Enter after typing', '入力後にEnter'],
    ['양 (음수 = 아래)', 'Amount (negative = down)', '量（マイナス = 下）'],
    ['찾아보기', 'Browse', '参照'],
    ['파일 고르는 중…', 'Choosing file…', 'ファイルを選択中…'],
    ['프로그램 (exe · 바로가기 · bat 등)', 'Program (exe · shortcut · bat, etc.)', 'プログラム（exe・ショートカット・batなど）'],
    ['실행 옵션 (선택)', 'Arguments (optional)', '起動オプション（任意）'],
    ['실행 후 대기 (ms)', 'Wait after launch (ms)', '起動後の待機 (ms)'],
    ['종료할 프로그램 (여러 개는 , 로 구분)', 'Programs to close (separate with ,)', '終了するプログラム（複数は , で区切る）'],
    ['열린 프로그램', 'Open programs', '起動中のプログラム'],
    ['종료 후 대기 (ms)', 'Wait after close (ms)', '終了後の待機 (ms)'],
    ['열린 창', 'Open windows', '開いているウィンドウ'],
    ['창 제목에 들어간 글자', 'Text in window title', 'ウィンドウタイトルに含まれる文字'],
    ['프로그램 이름 (선택)', 'Program name (optional)', 'プログラム名（任意）'],
    ['창 기다리기 (ms)', 'Wait for window (ms)', 'ウィンドウ待機 (ms)'],
    ['맨 앞으로 가져올 창', 'Window to bring to front', '最前面にするウィンドウ'],
    ['로블록스', 'Roblox', 'Roblox'],
    ['다른 창 (프로그램)', 'Another window (program)', '他のウィンドウ（プログラム）'],
    ['열린 프로그램에서 고르기…', 'Pick from open programs…', '起動中のプログラムから選択…'],
    ['열린 창에서 고르기…', 'Pick from open windows…', '開いているウィンドウから選択…'],
    ['2초 뒤 실행', 'Runs in 2s', '2秒後に実行'],
    ['F7 로 정지', 'stop with F7', 'F7で停止'],

    ['누르는 시간 (초)', 'Hold (s)', '押す時間（秒）'],
    ['간격 (초)', 'Gap (s)', '間隔（秒）'],
    ['시간 (초)', 'Time (s)', '時間（秒）'],
    ['글자 간격 (초)', 'Char gap (s)', '文字間隔（秒）'],
    ['실행 후 대기 (초)', 'Wait after launch (s)', '起動後の待機（秒）'],
    ['종료 후 대기 (초)', 'Wait after close (s)', '終了後の待機（秒）'],
    ['창 기다리기 (초)', 'Wait for window (s)', 'ウィンドウ待機（秒）'],
    ['드래그해서 순서 바꾸기', 'Drag to reorder', 'ドラッグで並べ替え'],
    ['입장 후 대기', 'Wait after entering', '参加後の待機'],
    ['복귀 후 Play 로 게임 입장이 감지된 뒤 첫 동작까지 (초)', 'From entering the game via Play after returning to the first action (s)', '復帰後Playでゲーム参加を検出してから最初の動作まで（秒）'],
    ['위치 템플릿', 'Position template', '位置テンプレート'],
    ['Play/Click to skip, 버튼 위치, OCR 영역을 한 번에 채움', 'Fills Play/Click to skip, button positions and the OCR area at once', 'Play/Click to skip、ボタン位置、OCR範囲を一括入力'],
    ['로블록스 화면 비율에 맞는 것 선택', 'pick the one matching your Roblox aspect ratio', 'Robloxの画面比率に合うものを選択'],
    ['적용', 'Apply', '適用'],
    ['한 번 더 누르면 덮어쓰기', 'Click again to overwrite', 'もう一度押すと上書き'],
    ['위치 템플릿을 쓸 수 있습니다', 'You can use a position template', '位置テンプレートを使えます'],
    ['로블록스를 <b>16:9</b> 화면(전체 화면 등)으로 쓴다면 <b>적용</b>을 눌러 버튼 위치와 OCR 영역을 한 번에 채울 수 있습니다.',
     'If Roblox runs at <b>16:9</b> (e.g. fullscreen), press <b>Apply</b> to fill in all button positions and the OCR area at once.',
     'Robloxを<b>16:9</b>（フルスクリーンなど）で使うなら、<b>適用</b>を押すとボタン位置とOCR範囲を一括で入力できます。'],
    ['선택 항목입니다. 채워진 위치 단계는 건너뜁니다. 화면 비율이 다르면 직접 지정해주세요.',
     'Optional. Steps for filled positions are skipped. If your aspect ratio differs, set them manually.',
     '任意項目です。入力済みの位置のステップはスキップされます。画面比率が違う場合は手動で指定してください。'],
    ['피드백', 'Feedback', 'フィードバック'],
    ['디스코드 서버에서 버그 · 건의', 'Report bugs & ideas on Discord', 'Discordサーバーでバグ・要望'],
    ['버그 · 건의 보내기', 'Send bugs · ideas', 'バグ・要望を送る'],
    ['버그 · 불편한 점 · 원하는 기능을 자유롭게 적어주세요. 만든 사람에게 바로 전달됩니다.', 'Write about bugs, problems or features you want. It goes straight to the creator.', 'バグ・不便な点・欲しい機能を自由に書いてください。制作者に直接届きます。'],
    ['내용을 입력해주세요', 'Please write something', '内容を入力してください'],
    ['답변 받을 디스코드 이름 (선택)', 'Your Discord name for a reply (optional)', '返信用のDiscord名（任意）'],
    ['보내기', 'Send', '送信'],
    ['보내는 중…', 'Sending…', '送信中…'],
    ['피드백을 보냈습니다. 감사합니다!', 'Feedback sent. Thank you!', 'フィードバックを送信しました。ありがとうございます！'],
    ['1500자 이하로 입력해주세요', 'Please keep it under 1500 characters', '1500文字以内で入力してください'],
    ['전송 실패', 'Send failed', '送信失敗'],
    ['로블록스가 갑자기 꺼졌습니다.', 'Roblox closed unexpectedly.', 'Robloxが突然終了しました。'],
    ['복귀 취소 — 대기', 'Return cancelled — waiting', '復帰キャンセル — 待機'],
    ['로블록스가 다시 켜지지 않음 — 매크로 복귀 실행', "Roblox didn't come back — running macro return", 'Robloxが再起動しない — マクロ復帰を実行'],
    ['복귀 취소 — 일부러 끈 것으로 보고 대기', 'Return cancelled — treated as intentional, waiting', '復帰キャンセル — 意図的な終了として待機'],
    ['로블록스 꺼짐', 'Roblox closed', 'Roblox終了'],
    ['완전 삭제', 'Uninstall', '完全削除'],
    ['Acrux 데이터 전부 지우기', 'Delete all Acrux data', 'Acruxのデータをすべて削除'],
    ['Acrux 매크로를 완전히 삭제할까요?', 'Completely remove Acrux macro?', 'Acruxマクロを完全に削除しますか？'],
    ['설정 · 위치 · 템플릿 · 기록과 내려받은 프로그램 파일이 전부 지워지고, 이 창이 닫힙니다.', 'All settings, positions, templates, history and downloaded program files will be deleted, and this window will close.', '設定・位置・テンプレート・履歴とダウンロードしたプログラムファイルがすべて削除され、このウィンドウが閉じます。'],
    ['실행기(Acrux.exe)는 그대로 남습니다. 다시 실행하면 처음부터 새로 설치됩니다.', 'The launcher (Acrux.exe) stays. Running it again installs everything fresh.', 'ランチャー（Acrux.exe）は残ります。もう一度起動すると最初から新しくインストールされます。'],
    ['취소', 'Cancel', 'キャンセル'],
    ['삭제 중… 창이 곧 닫힙니다', 'Deleting… the window will close shortly', '削除中… まもなくウィンドウが閉じます'],
    ['삭제할 폴더를 찾을 수 없음', "Couldn't find the folder to delete", '削除するフォルダが見つかりません'],
    ['Acrux 완전 삭제 — 창을 닫고 데이터를 지웁니다', 'Uninstalling Acrux — closing the window and deleting data', 'Acrux完全削除 — ウィンドウを閉じてデータを削除します'],
    // 튜토리얼 — 공통
    ['스킵', 'Skip', 'スキップ'],
    ['이 단계 건너뛰기', 'Skip this step', 'このステップをスキップ'],
    ['이전', 'Back', '戻る'],
    ['다음', 'Next', '次へ'],
    ['확인', 'OK', 'OK'],
    ['나중에', 'Later', '後で'],
    ['모든 튜토리얼 완료', 'All tutorials done', 'すべてのチュートリアル完了'],
    ['입력해야 다음으로 넘어갈 수 있습니다', 'Fill this in to continue', '入力すると次へ進めます'],
    ['설정이 완료되었습니다', 'Setup complete', '設定が完了しました'],
    ['필수 항목입니다.', 'This is required.', '必須項目です。'],
    ['선택 항목입니다. 필요 없으면 이 단계를 건너뛰어주세요.', "Optional. Skip this step if you don't need it.", '任意項目です。不要ならこのステップをスキップしてください。'],
    ['선택 항목입니다. 기본값 그대로 써도 됩니다.', 'Optional. The defaults are fine.', '任意項目です。初期値のままでも大丈夫です。'],
    ['감시 대상 설정', 'Watch target setup', '監視対象の設定'],
    ['감시 대상 설정 튜토리얼을 건너뛰었습니다', 'Skipped the watch target tutorial', '監視対象設定のチュートリアルをスキップしました'],
    ['감시 대상 탭에서 다시 볼 수 있습니다', 'You can view it again in the Watch targets tab', '監視対象タブでもう一度見られます'],
    ['바이옴 매크로 설정 튜토리얼은 다음에 다시 표시됩니다', 'The biome macro tutorial will show again next time', 'バイオームマクロ設定のチュートリアルは次回また表示されます'],
    ['오토 팝핑 설정 튜토리얼을 건너뛰었습니다', 'Skipped the auto popping tutorial', 'オートポッピング設定のチュートリアルをスキップしました'],
    ['설정을 끝내기 전까지 매크로는 시작되지 않습니다', "The macro won't start until setup is finished", '設定を終えるまでマクロは開始されません'],
    ['매크로 복귀 설정 튜토리얼을 건너뛰었습니다', 'Skipped the macro return tutorial', 'マクロ復帰設定のチュートリアルをスキップしました'],

    // 튜토리얼 — 감시 대상
    ['감시 대상 설정이 필요합니다', 'Watch targets need to be set', '監視対象の設定が必要です'],
    ['링크를 감지할 <b>디스코드 채널</b> 또는 <b>서버</b>를 먼저 등록해야 작동합니다.',
     'Register the <b>Discord channel</b> or <b>server</b> to watch for links first.',
     'リンクを検知する<b>Discordチャンネル</b>または<b>サーバー</b>を先に登録してください。'],
    ['등록한 곳에 올라온 비공개 서버 링크만 감지합니다.', 'Only private server links posted there are detected.', '登録した場所に投稿されたプライベートサーバーリンクのみ検知します。'],
    ['디스코드 개발자 모드를 켜주세요', 'Turn on Discord Developer Mode', 'Discordの開発者モードをオンにしてください'],
    ['디스코드 왼쪽 아래 <b>톱니바퀴 (사용자 설정)</b>를 눌러주세요.', 'Click the <b>gear (User Settings)</b> at the bottom left of Discord.', 'Discord左下の<b>歯車（ユーザー設定）</b>をクリックしてください。'],
    ['왼쪽 목록에서 <b>고급</b>을 눌러주세요.', 'Click <b>Advanced</b> in the left list.', '左のリストで<b>詳細設定</b>をクリックしてください。'],
    ['<b>개발자 모드</b>를 켜주세요.', 'Turn on <b>Developer Mode</b>.', '<b>開発者モード</b>をオンにしてください。'],
    ['ID 복사 메뉴를 보기 위해 필요하며, 한 번만 하면 됩니다.', 'Needed for the Copy ID menu. You only need to do this once.', '「IDをコピー」メニューを表示するために必要で、一度だけで大丈夫です。'],
    ['ID를 복사해주세요', 'Copy the ID', 'IDをコピーしてください'],
    ['<b>채널 하나만</b> 감시하려면 링크가 올라오는 채널 이름을 <b>우클릭 → ID 복사하기</b>를 눌러주세요.',
     'To watch <b>a single channel</b>, <b>right-click → Copy ID</b> on the channel where links are posted.',
     '<b>チャンネル1つだけ</b>を監視するなら、リンクが投稿されるチャンネル名を<b>右クリック → IDをコピー</b>してください。'],
    ['<b>서버 전체</b>를 감시하려면 왼쪽 서버 아이콘을 <b>우클릭 → ID 복사하기</b>를 눌러주세요.',
     'To watch <b>a whole server</b>, <b>right-click → Copy ID</b> on the server icon on the left.',
     '<b>サーバー全体</b>を監視するなら、左のサーバーアイコンを<b>右クリック → IDをコピー</b>してください。'],
    ['스레드에 올라온 링크는 감지하지 않습니다.', 'Links posted in threads are not detected.', 'スレッドに投稿されたリンクは検知しません。'],
    ['ID를 붙여넣고 추가해주세요', 'Paste the ID and add it', 'IDを貼り付けて追加してください'],
    ['복사한 채널 ID를 이 칸에 붙여넣고 <b>추가</b>를 눌러주세요.', 'Paste the copied channel ID here and click <b>Add</b>.', 'コピーしたチャンネルIDをこの欄に貼り付けて<b>追加</b>をクリックしてください。'],
    ['서버 ID는 왼쪽 서버 칸에 넣어주세요. 이름은 디스코드에서 자동으로 가져옵니다.', 'Put server IDs in the Server box on the left. Names are fetched from Discord automatically.', 'サーバーIDは左のサーバー欄に入れてください。名前はDiscordから自動で取得します。'],
    ['감시 대상 등록이 끝났습니다.', 'Watch targets are registered.', '監視対象の登録が完了しました。'],
    ['메인 화면 <b>오토 스나이핑</b>의 <b>시작</b> 버튼을 누르면 작동합니다.', 'Press <b>Start</b> on <b>Auto sniping</b> on the main screen to begin.', 'メイン画面の<b>オートスナイプ</b>の<b>開始</b>ボタンを押すと作動します。'],
    ['필터 탭에서 바이옴을 선택하면 원하는 바이옴만 감지합니다.', 'Pick biomes in the Filter tab to detect only the ones you want.', 'フィルタータブでバイオームを選ぶと、そのバイオームだけ検知します。'],

    // 튜토리얼 — 바이옴 매크로
    ['바이옴 매크로 설정이 필요합니다', 'Biome macro setup needed', 'バイオームマクロの設定が必要です'],
    ['로블록스 로그로 현재 바이옴을 인식하고, 바이옴이 바뀌면 <b>디스코드 웹후크</b>로 알림을 보냅니다.',
     'Reads the current biome from Roblox logs and sends a <b>Discord webhook</b> alert when it changes.',
     'Robloxのログで現在のバイオームを検出し、変わると<b>Discord Webhook</b>で通知します。'],
    ['<b>플레이어 이름</b>은 꼭 입력해야 하며, <b>브섭 링크</b>와 <b>웹후크</b>는 선택입니다.',
     '<b>Player name</b> is required; <b>PS link</b> and <b>webhook</b> are optional.',
     '<b>プレイヤー名</b>は必須で、<b>PSリンク</b>と<b>Webhook</b>は任意です。'],
    ['플레이어 이름을 입력해주세요', 'Enter your player name', 'プレイヤー名を入力してください'],
    ['로블록스 <b>실제 닉네임</b>을 입력해주세요. 디스플레이 이름이 아닌 <b>@ 뒤의 이름</b>입니다.',
     'Enter your Roblox <b>username</b> — not your display name, but <b>the name after @</b>.',
     'Robloxの<b>ユーザー名</b>を入力してください。表示名ではなく<b>@の後の名前</b>です。'],
    ['게임 속 닉네임과 같아야 바이옴을 감지합니다. 필수 항목입니다.', 'It must match your in-game username to detect biomes. This is required.', 'ゲーム内のユーザー名と一致しないとバイオームを検出できません。必須項目です。'],
    ['브섭 링크를 입력해주세요', 'Enter your PS link', 'PSリンクを入力してください'],
    ['바이옴 알림에 함께 표시할 <b>비공개 서버 링크</b>를 붙여넣어주세요.', 'Paste the <b>private server link</b> to show in biome alerts.', 'バイオーム通知に表示する<b>プライベートサーバーリンク</b>を貼り付けてください。'],
    ['알림을 디스코드로 받으려면 왼쪽 목록에서 강조된 <b>디스코드 웹후크</b>를 눌러주세요.', 'To get alerts on Discord, click the highlighted <b>Discord webhook</b> in the left list.', 'Discordで通知を受け取るなら、左のリストで強調された<b>Discord Webhook</b>をクリックしてください。'],
    ['선택 항목입니다. 알림이 필요 없으면 이 단계를 건너뛰어주세요.', "Optional. Skip this step if you don't need alerts.", '任意項目です。通知が不要ならこのステップをスキップしてください。'],
    ['웹후크 주소를 입력해주세요', 'Enter the webhook URL', 'Webhook URLを入力してください'],
    ['알림을 받을 디스코드 채널의 <b>톱니바퀴 (채널 편집)</b>를 눌러주세요.', 'Click the <b>gear (Edit Channel)</b> of the Discord channel for alerts.', '通知を受け取るDiscordチャンネルの<b>歯車（チャンネルの編集）</b>をクリックしてください。'],
    ['<b>연동 → 웹후크 → 새 웹후크</b>를 누른 뒤 <b>웹후크 URL 복사</b>를 눌러주세요.', 'Go to <b>Integrations → Webhooks → New Webhook</b>, then click <b>Copy Webhook URL</b>.', '<b>連携サービス → ウェブフック → 新しいウェブフック</b>を押し、<b>ウェブフックURLをコピー</b>をクリックしてください。'],
    ['복사한 주소를 <b>웹후크 1</b> 칸에 붙여넣어주세요.', 'Paste the URL into the <b>Webhook 1</b> box.', 'コピーしたURLを<b>Webhook 1</b>欄に貼り付けてください。'],
    ['튜토리얼이 끝난 뒤 위쪽 [테스트 전송]으로 확인할 수 있습니다. 선택 항목입니다.', 'After the tutorial, you can check it with [Send test] at the top. Optional.', 'チュートリアル後、上の［テスト送信］で確認できます。任意項目です。'],
    ['바이옴 매크로 설정이 끝났습니다.', 'Biome macro setup is done.', 'バイオームマクロの設定が完了しました。'],
    ['메인 화면 <b>바이옴 매크로</b>의 <b>시작</b> 버튼이나, 설정 화면 위쪽 스위치를 켜면 알림을 보냅니다.',
     'Press <b>Start</b> on <b>Biome macro</b> on the main screen, or turn on the switch at the top of its settings, to send alerts.',
     'メイン画面の<b>バイオームマクロ</b>の<b>開始</b>ボタン、または設定画面上部のスイッチをオンにすると通知します。'],
    ['프로그램을 켤 때마다 꺼진 상태로 시작합니다.', 'It starts off every time you open the program.', 'プログラムを起動するたびにオフの状態で始まります。'],

    // 튜토리얼 — 오토 팝핑
    ['오토 팝핑 설정이 필요합니다', 'Auto popping setup needed', 'オートポッピングの設定が必要です'],
    ['스나이핑으로 접속한 뒤 <b>Play</b> 버튼을 누르고, 레어 바이옴이면 <b>포션을 자동으로 사용</b>합니다.',
     'After sniping into a server, it presses <b>Play</b> and, in a rare biome, <b>uses potions automatically</b>.',
     'スナイプで参加した後に<b>Play</b>ボタンを押し、レアバイオームなら<b>ポーションを自動で使用</b>します。'],
    ['버튼 위치와 OCR 영역은 꼭 지정해야 하며, 접속 전 동작 · 팝핑 바이옴 · 딜레이 · 템플릿은 선택입니다.',
     'Button positions and the OCR area are required; Before join, popping biomes, delays and templates are optional.',
     'ボタン位置とOCR範囲は必須で、参加前の動作・ポッピング対象バイオーム・待ち時間・テンプレートは任意です。'],
    ['위치를 지정할 때는 로블록스를 켜두고, 해당 버튼이 보이는 화면에서 진행해주세요.', 'When setting positions, keep Roblox open on a screen where that button is visible.', '位置を指定する時はRobloxを起動し、そのボタンが見える画面で行ってください。'],
    ['게임에 접속하기 직전에 할 동작을 정합니다. 왼쪽 목록에서 강조된 <b>접속 전 동작</b>을 눌러주세요.',
     'Set what to do right before joining. Click the highlighted <b>Before join</b> in the left list.',
     'ゲームに参加する直前の動作を決めます。左のリストで強調された<b>参加前の動作</b>をクリックしてください。'],
    ['접속 전 동작을 추가해주세요', 'Add before-join actions', '参加前の動作を追加してください'],
    ['예: <b>+ 프로그램 강제 종료</b>로 방해되는 프로그램을 끄거나, <b>+ 키 입력 / + 키 조합</b>으로 단축키를 누를 수 있습니다.',
     'e.g. use <b>+ Force-close program</b> to close programs that get in the way, or <b>+ Key press / + Key combo</b> to press shortcuts.',
     '例: <b>+ プログラム強制終了</b>で邪魔なプログラムを終了したり、<b>+ キー入力 / + キーの組み合わせ</b>でショートカットを押せます。'],
    ['위에서부터 순서대로 실행한 뒤 접속합니다. 선택 항목입니다.', 'Runs from the top in order, then joins. Optional.', '上から順番に実行してから参加します。任意項目です。'],
    ['<b>위치 지정</b>을 누르면 로블록스 화면이 앞으로 나옵니다. 로블록스의 <b>Click to skip</b> 버튼을 한 번 클릭해주세요.',
     'Click <b>Set position</b> and Roblox comes to the front. Click Roblox\'s <b>Click to skip</b> button once.',
     '<b>位置を指定</b>を押すとRobloxが前面に出ます。Robloxの<b>Click to skip</b>ボタンを一度クリックしてください。'],
    ['Play 버튼과 번갈아 클릭합니다. 필수 항목입니다. Esc 로 취소할 수 있습니다.', "It's clicked alternately with Play. This is required. Press Esc to cancel.", 'Playボタンと交互にクリックします。必須項目です。Escでキャンセルできます。'],
    ['같은 방법으로 <b>Play</b> 버튼 위치를 지정해주세요.', 'Set the <b>Play</b> button position the same way.', '同じ方法で<b>Play</b>ボタンの位置を指定してください。'],
    ['클릭 간격과 대기 시간을 확인해주세요', 'Check the click interval and wait times', 'クリック間隔と待機時間を確認してください'],
    ['<b>클릭 간격</b>: Play ↔ Click to skip 번갈아 누르는 간격 (기본 0.15초)',
     '<b>Click interval</b>: gap between alternating Play ↔ Click to skip clicks (default 0.15s)',
     '<b>クリック間隔</b>: Play ↔ Click to skipを交互に押す間隔（初期値0.15秒）'],
    ['<b>접속 후 대기</b>: 로블록스 창이 뜬 뒤 첫 클릭까지 · <b>최대 시도 시간</b>: 이 안에 입장이 안 되면 중지',
     "<b>Wait after launch</b>: from the Roblox window opening to the first click · <b>Max try time</b>: stops if you haven't entered by then",
     '<b>起動後の待機</b>: Robloxウィンドウ表示から最初のクリックまで・<b>最大試行時間</b>: この時間内に入れなければ停止'],
    ['OCR 영역을 지정해주세요', 'Set the OCR area', 'OCR範囲を指定してください'],
    ['<b>드래그로 지정</b>을 누른 뒤, 검색 결과 첫 칸의 <b>아이템 이름과 개수</b>(예: Warp Potion x23)가 들어가도록 드래그해주세요.',
     'Click <b>Drag to set</b>, then drag over the first search result so it covers the <b>item name and count</b> (e.g. Warp Potion x23).',
     '<b>ドラッグで指定</b>を押してから、検索結果の最初の欄の<b>アイテム名と個数</b>（例: Warp Potion x23）が入るようにドラッグしてください。'],
    ['필수 항목입니다. <b>OCR 테스트</b>로 제대로 읽히는지 확인할 수 있습니다.', 'This is required. Use <b>OCR test</b> to check it reads correctly.', '必須項目です。<b>OCRテスト</b>で正しく読めるか確認できます。'],
    ['오토 팝핑을 할 바이옴을 고릅니다. 왼쪽 목록에서 강조된 <b>팝핑 바이옴 설정</b>을 눌러주세요.',
     'Choose which biomes to auto pop in. Click the highlighted <b>Popping biomes</b> in the left list.',
     'オートポッピングするバイオームを選びます。左のリストで強調された<b>ポッピング対象バイオーム</b>をクリックしてください。'],
    ['선택 항목입니다. 기본은 세 바이옴 모두 켜져 있습니다.', 'Optional. All three biomes are on by default.', '任意項目です。初期状態では3つのバイオームすべてオンです。'],
    ['팝핑할 바이옴을 켜고 꺼주세요', 'Turn biomes on or off', 'ポッピングするバイオームをオン・オフしてください'],
    ['켜진 바이옴에서만 오토 팝핑을 합니다. 꺼진 바이옴에 들어가면 팝핑 없이 바로 매크로 복귀를 합니다.',
     "Auto popping only runs in biomes that are on. In a biome that's off, it skips popping and returns right away.",
     'オンのバイオームでのみオートポッピングします。オフのバイオームに入ると、ポッピングせずにすぐマクロ復帰します。'],
    ['동작 사이 대기 시간을 바꾸려면 왼쪽 목록에서 강조된 <b>오토 팝핑 딜레이 설정</b>을 눌러주세요.',
     'To change the wait between actions, click the highlighted <b>Auto popping delays</b> in the left list.',
     '動作間の待ち時間を変えるなら、左のリストで強調された<b>オートポッピングの待ち時間</b>をクリックしてください。'],
    ['딜레이를 조정해주세요', 'Adjust the delays', '待ち時間を調整してください'],
    ['필요한 항목의 시간(초)을 바꿔주세요. 바꾼 값은 바로 저장됩니다.', 'Change the times (seconds) you need. Changes save right away.', '必要な項目の時間（秒）を変更してください。変更はすぐ保存されます。'],
    ['바이옴별로 사용할 포션 목록을 확인합니다. 왼쪽 목록에서 강조된 <b>Glitched</b>를 눌러주세요.',
     'Check the potion list for each biome. Click the highlighted <b>Glitched</b> in the left list.',
     'バイオームごとに使うポーションのリストを確認します。左のリストで強調された<b>Glitched</b>をクリックしてください。'],
    ['선택 항목입니다. 템플릿이 없는 바이옴에서는 오토 팝핑을 하지 않습니다.', "Optional. Biomes without a template aren't auto popped.", '任意項目です。テンプレートがないバイオームではオートポッピングしません。'],
    ['사용할 포션을 확인해주세요', 'Check the potions to use', '使用するポーションを確認してください'],
    ['기본 템플릿(레어 바이옴 자동 팝핑)이 미리 들어 있습니다. <b>+ 포션 추가</b>로 추가하고, <b>✕</b>로 삭제할 수 있습니다.',
     'The default template (rare biome auto popping) is already filled in. Add with <b>+ Add potion</b> and remove with <b>✕</b>.',
     'デフォルトテンプレート（レアバイオーム自動ポッピング）があらかじめ入っています。<b>+ ポーション追加</b>で追加、<b>✕</b>で削除できます。'],
    ['위쪽일수록 먼저 사용합니다. Cyberspace · Dreamspace 도 같은 방법으로 바꿀 수 있습니다. 선택 항목입니다.',
     'Higher ones are used first. Cyberspace · Dreamspace can be changed the same way. Optional.',
     '上にあるほど先に使用します。Cyberspace・Dreamspaceも同じ方法で変更できます。任意項目です。'],
    ['오토 팝핑 설정이 끝났습니다.', 'Auto popping setup is done.', 'オートポッピングの設定が完了しました。'],
    ['링크로 접속하면 Play 버튼을 누르고, 켜진 레어 바이옴이면 포션을 자동으로 사용합니다.', 'When joining from a link, it presses Play and uses potions automatically in rare biomes that are on.', 'リンクから参加するとPlayボタンを押し、オンのレアバイオームならポーションを自動で使用します。'],
    ['각 바이옴 탭의 <b>이 템플릿 테스트</b>로 바로 확인할 수 있습니다. 정지는 F7 입니다.', 'Try it right away with <b>Test this template</b> in each biome tab. Stop with F7.', '各バイオームタブの<b>このテンプレートをテスト</b>ですぐ確認できます。停止はF7です。'],

    // 튜토리얼 — 매크로 복귀
    ['매크로 복귀 설정이 필요합니다', 'Macro return setup needed', 'マクロ復帰の設定が必要です'],
    ['바이옴이 끝나거나 오토 팝핑이 어떤 이유로든 끝나면, <b>로블록스를 모두 종료</b>하고 <b>내 서버로 돌아갑니다</b>.',
     'When the biome ends or auto popping stops for any reason, it <b>closes all Roblox</b> and <b>returns to your server</b>.',
     'バイオームが終わるかオートポッピングが何らかの理由で終わると、<b>Robloxをすべて終了</b>して<b>自分のサーバーに戻ります</b>。'],
    ['<b>내 브섭 링크</b>는 꼭 입력해야 하며, <b>복귀 후 동작</b>은 선택입니다.', '<b>My PS link</b> is required; <b>After return</b> is optional.', '<b>自分のPSリンク</b>は必須で、<b>復帰後の動作</b>は任意です。'],
    ['내 브섭 링크를 입력해주세요', 'Enter your PS link', '自分のPSリンクを入力してください'],
    ['돌아갈 <b>내 비공개 서버 링크</b>를 붙여넣어주세요.', 'Paste <b>your private server link</b> to return to.', '戻る先の<b>自分のプライベートサーバーリンク</b>を貼り付けてください。'],
    ['바이옴 매크로 설정에 이미 넣었다면 <b>가져오기</b>로 그대로 쓸 수 있습니다. 필수 항목입니다.', 'If you already entered it in Biome macro settings, just use <b>Import</b>. This is required.', 'バイオームマクロ設定に入力済みなら<b>取り込む</b>でそのまま使えます。必須項目です。'],
    ['내 서버에 들어간 뒤 할 동작을 정합니다. 왼쪽 목록에서 강조된 <b>복귀 후 동작</b>을 눌러주세요.',
     'Set what to do after entering your server. Click the highlighted <b>After return</b> in the left list.',
     '自分のサーバーに入った後の動作を決めます。左のリストで強調された<b>復帰後の動作</b>をクリックしてください。'],
    ['복귀 후 동작을 추가해주세요', 'Add after-return actions', '復帰後の動作を追加してください'],
    ['키 입력 · 키 조합(Ctrl+L 등) · 클릭 · 프로그램 실행 · 창 맨 앞으로 등을 순서대로 추가할 수 있습니다.',
     'You can add key presses · key combos (Ctrl+L, etc.) · clicks · running programs · bringing windows to front, in order.',
     'キー入力・キーの組み合わせ（Ctrl+Lなど）・クリック・プログラム実行・ウィンドウを最前面へ などを順番に追加できます。'],
    ['게임 입장이 감지되고 <b>입장 후 대기</b>(기본 7.5초) 뒤에 시작합니다. 키 입력은 맨 앞에 있는 창으로 들어가니, 필요하면 <b>창 맨 앞으로</b>를 먼저 넣어주세요. 왼쪽 <b>II</b>를 끌어서 순서를 바꿀 수 있습니다.',
     'Starts after <b>Wait after entering</b> (default 7.5s) once you\'re in game. Key presses go to the window in front, so add <b>Bring window to front</b> first if needed. Drag the <b>II</b> on the left to reorder.',
     'ゲーム参加を検出し、<b>参加後の待機</b>（初期値7.5秒）の後に開始します。キー入力は最前面のウィンドウに送られるので、必要なら先に<b>ウィンドウを最前面へ</b>を入れてください。左の<b>II</b>をドラッグして順番を変えられます。'],
    ['매크로 복귀 설정이 끝났습니다.', 'Macro return setup is done.', 'マクロ復帰の設定が完了しました。'],
    ['위쪽 <b>복귀 테스트</b>로 바로 확인할 수 있습니다. 정지는 F7 입니다.', 'Try it right away with <b>Test return</b> at the top. Stop with F7.', '上の<b>復帰テスト</b>ですぐ確認できます。停止はF7です。'],

    // 프로그램(파이썬) 쪽 상태 · 로그 · 오류
    ['디스코드 연결 중', 'Connecting to Discord', 'Discordに接続中'],
    ['디스코드 없음', 'Discord not found', 'Discordが見つかりません'],
    ['디코 재시작 필요', 'Discord restart needed', 'Discordの再起動が必要'],
    ['디스코드 재시작 중', 'Restarting Discord', 'Discordを再起動中'],
    ['디스코드 실행 중', 'Launching Discord', 'Discordを起動中'],
    ['포트 안 열림', 'Port not open', 'ポートが開きません'],
    ['디스코드 창 찾는 중', 'Finding Discord window', 'Discordウィンドウを検索中'],
    ['Flux 연결 대기', 'Waiting for Flux', 'Flux接続待ち'],
    ['모든 채널 감시 중', 'Watching all channels', '全チャンネル監視中'],
    ['열린 채널만 감시', 'Watching the open channel only', '開いているチャンネルのみ監視'],
    ['hook.js 없음', 'hook.js missing', 'hook.jsがありません'],
    ['재연결 대기', 'Waiting to reconnect', '再接続待ち'],
    ['매크로 복귀 완료 — 내 서버 입장', 'Macro return done — entered your server', 'マクロ復帰完了 — 自分のサーバーに参加'],
    ['켜진 동작이 없음', 'No actions turned on', 'オンの動作がありません'],
    ['매크로 복귀 설정에 내 브섭 링크가 없음', 'No PS link in Macro return settings', 'マクロ復帰設定に自分のPSリンクがありません'],
    ['복귀할 브섭 링크 먼저 입력', 'Enter the PS link to return to first', '先に復帰先のPSリンクを入力してください'],
    ['이미 복귀 중', 'Already returning', 'すでに復帰中'],
    ['알 수 없는 항목', 'Unknown item', '不明な項目'],
    ['OCR 영역 먼저 지정', 'Set the OCR area first', '先にOCR範囲を指定してください'],
    ['OCR 패키지가 설치되지 않음 (run.bat 으로 실행 필요)', 'OCR package not installed (run with run.bat)', 'OCRパッケージ未インストール（run.batで起動してください）'],
    ['알 수 없는 바이옴', 'Unknown biome', '不明なバイオーム'],
    ['포션 목록이 비어 있음', 'Potion list is empty', 'ポーションリストが空です'],
    ['웹후크 주소 없음', 'No webhook URL', 'Webhook URLがありません'],
    ['디스코드 웹후크 주소 형식이 아님', 'Not a Discord webhook URL', 'Discord WebhookのURL形式ではありません'],
    ['취소됨', 'Cancelled', 'キャンセルしました'],
    ['지정 실패', 'Failed to set', '指定失敗'],
    ['시간 초과', 'Timed out', 'タイムアウト'],
    ['영역이 없음', 'No area', '範囲がありません'],
    ['오토 팝핑 설정 필요', 'Auto popping setup needed', 'オートポッピング設定が必要'],
    ['작동 시작', 'Started', '作動開始'],
    ['작동 중지 — 감지만 계속', 'Stopped — still detecting', '作動停止 — 検知のみ継続'],
    ['윈도우 OCR', 'Windows OCR', 'Windows OCR'],
    ['디스코드가 디버그 포트 없이 실행 중 — 디스코드 종료 또는 \'자동 재시작\' 설정 필요',
     "Discord is running without the debug port — close Discord or turn on 'Auto restart'",
     'Discordがデバッグポートなしで起動中 — Discordを終了するか「自動再起動」をオンにしてください'],
    ['디스코드를 디버그 포트와 함께 재시작하는 중…', 'Restarting Discord with the debug port…', 'デバッグポート付きでDiscordを再起動中…'],
    ['디스코드 실행 중…', 'Launching Discord…', 'Discordを起動中…'],
    ['디버그 포트 열기 실패 — 디스코드가 이 옵션을 막았을 가능성 있음', 'Failed to open the debug port — Discord may be blocking it', 'デバッグポートを開けません — Discordがこのオプションをブロックしている可能性'],
    ['로블록스 클라이언트 강제 종료', 'Force-closed the Roblox client', 'Robloxクライアントを強制終了'],
    ['피드백 서버 준비 중 · 디스코드 서버로 알려주세요', 'Feedback server not ready yet · please tell us on the Discord server', 'フィードバックサーバー準備中 · Discordサーバーでお知らせください'],
    ['잠시 뒤에 다시 보내주세요', 'Please try again in a moment', 'しばらくしてからもう一度送信してください'],
    ['비공개 서버 링크 형식이 아니라서 열지 않음', 'Not a private server link — not opened', 'プライベートサーバーのリンク形式ではないため開きません'],
    ['로블록스 비공개 서버 링크 형식이 아님', 'Not a Roblox private server link', 'Robloxのプライベートサーバーリンク形式ではありません'],
    ['디스코드가 예전 방식(웹페이지도 접속 가능)으로 켜져 있음 → 안전한 방식으로 다시 켬', 'Discord is running in the old mode (reachable from web pages) → restarting it the safe way', 'Discordが旧方式（Webページからも接続可能）で起動中 → 安全な方式で再起動'],
    ['디스코드가 예전 방식(웹페이지도 접속 가능)으로 켜져 있음 — 디스코드를 껐다가 다시 켜주세요 (자동 재시작 꺼짐)', 'Discord is running in the old mode (reachable from web pages) — please close and reopen Discord (auto restart is off)', 'Discordが旧方式（Webページからも接続可能）で起動中 — Discordを再起動してください（自動再起動オフ）'],
    ['디스코드를 보통 모드로 다시 켜는 중 (디버그 포트 닫기)', 'Restarting Discord in normal mode (closing the debug port)', 'Discordを通常モードで再起動中（デバッグポートを閉じる）'],
    ['디스코드를 다시 켬', 'Restarting Discord', 'Discordを再起動'],
    ['연결 안 함', 'Not connecting', '接続しません'],
    ['포트 확인 실패', 'Port check failed', 'ポート確認失敗'],
    ['디버그 포트 안전 확인이 계속 실패해서 디스코드를 다시 켜지 않음 — 디스코드와 Acrux 를 껐다가 다시 켜주세요', 'Debug port safety check keeps failing, so Discord won\'t be restarted — please close and reopen Discord and Acrux', 'デバッグポートの安全確認が失敗し続けるため、Discordを再起動しません — DiscordとAcruxを再起動してください'],
    ['디스코드 창 ID 가 이상함', 'Invalid Discord window ID', 'DiscordウィンドウIDが不正です'],
    ['딥링크 변환 실패 → 브라우저로 열기', "Couldn't convert to a deep link → opening in browser", 'ディープリンクに変換できません → ブラウザで開く'],
    ['로블록스 클라이언트 전부 종료', 'Closed all Roblox clients', 'Robloxクライアントをすべて終了'],
    ['디스코드 메인 창을 찾을 수 없음 (로그인 화면이면 로그인 필요)', 'Discord main window not found (log in if on the login screen)', 'Discordのメインウィンドウが見つかりません（ログイン画面ならログインしてください）'],
    ['Flux 연결됨 — 모든 채널 감시 중', 'Flux connected — watching all channels', 'Flux接続 — 全チャンネル監視中'],
    ['Flux 연결 실패 — 화면에 열린 채널만 감시 (DOM)', 'Flux failed — watching only the open channel (DOM)', 'Flux接続失敗 — 画面に開いているチャンネルのみ監視（DOM）'],
    ['디스코드 연결 끊김', 'Disconnected from Discord', 'Discordとの接続が切れました'],
    ['디스코드 연결 끊김 (앱 종료/업데이트 재시작)', 'Disconnected from Discord (app closed / update restart)', 'Discordとの接続が切れました（アプリ終了/アップデート再起動）'],
    ['hook.js 가 같은 폴더에 없음', "hook.js isn't in the same folder", 'hook.jsが同じフォルダにありません'],
    ['다시 연결 시도…', 'Reconnecting…', '再接続中…'],
    ['다른 서버 접속 (스나이핑) — 이번 접속 동안 바이옴 웹후크 안 보냄', 'Joined another server (snipe) — no biome webhooks this session', '別サーバーに参加（スナイプ）— この接続中はバイオームWebhookを送りません'],
    ['로블록스 접속 끊김 감지 (로그)', 'Roblox disconnect detected (log)', 'Robloxの切断を検出（ログ）'],
    ['오토 팝핑 정지', 'Auto popping stopped', 'オートポッピング停止'],
    ['Inventory 열기', 'Opening Inventory', 'Inventoryを開く'],
    ['로블록스 접속 끊김 — 오토 팝핑 종료', 'Disconnected from Roblox — auto popping finished', 'Roblox切断 — オートポッピング終了'],
    ['로블록스 창이 사라짐 — 오토 팝핑 종료', 'Roblox window gone — auto popping finished', 'Robloxウィンドウが消えた — オートポッピング終了'],
    ['로블록스 창 기다리는 중', 'Waiting for the Roblox window', 'Robloxウィンドウ待機中'],
    ['로블록스 창이 안 떠서 Play 클릭 취소', 'Roblox window never opened — Play clicking cancelled', 'Robloxウィンドウが開かずPlayクリックを中止'],
    ['맨 앞으로 가져옴', 'brought to front', '最前面に表示'],
    ['맨 앞으로 못 가져옴', "couldn't bring to front", '最前面にできず'],
    ['앞에 있음', 'in front', '前面'],
    ['앞으로 못 가져옴', "couldn't bring to front", '前面にできず'],
    ['입장 확인 중', 'checking entry', '参加確認中'],
    ['Play 클릭 정지', 'Play clicking stopped', 'Playクリック停止'],
    ['매크로 복귀 안 함 — 복귀할 브섭 링크가 없음 (매크로 복귀 설정에서 입력)', 'No macro return — no PS link set (enter it in Macro return settings)', 'マクロ復帰なし — 復帰先のPSリンクがありません（マクロ復帰設定で入力）'],
    ['로블록스 종료', 'Closing Roblox', 'Robloxを終了'],
    ['1초 대기', 'Waiting 1s', '1秒待機'],
    ['매크로 복귀 정지', 'Macro return stopped', 'マクロ復帰停止'],
    ['로블록스 창을 찾을 수 없음', 'Roblox window not found', 'Robloxウィンドウが見つかりません'],
    ['클릭 위치가 지정되지 않음', 'Click position not set', 'クリック位置が未指定'],
    ['종료할 프로그램 이름이 없음', 'No program name to close', '終了するプログラム名がありません'],
    ['실행할 프로그램이 지정되지 않음', 'No program set to run', '実行するプログラムが未指定'],
    ['(비어 있음)', '(empty)', '（空）'],
    ['윈도우 OCR 언어 없음 (윈도우 설정 → 언어에서 한국어/영어 언어팩 설치 필요)', 'No Windows OCR language (install a Korean/English language pack in Windows Settings → Language)', 'Windows OCRの言語がありません（Windowsの設定 → 言語で韓国語/英語の言語パックをインストール）'],
    ['OCR 시간 초과', 'OCR timed out', 'OCRタイムアウト'],
    ['OCR 응답 없음 (시간 초과)', 'No OCR response (timed out)', 'OCRの応答なし（タイムアウト）'],
    ['접속 후 처리', 'Post-join', '参加後の処理'],
    ['주입', 'Injection', '注入'],
    ['바이옴 감시', 'Biome watcher', 'バイオーム監視'],
    ['복귀 연결', 'Return handoff', '復帰連携'],
    ['Play 클릭', 'Play click', 'Playクリック'],
    ['매크로 복귀', 'Macro return', 'マクロ復帰'],
    [' (안정화)', ' (stable)', '（安定）'],
    [' (재시도)', ' (retry)', '（再試行）'],
    [' (실행 중 아님)', ' (not running)', '（起動していません）'],
    ['직접 실행으로는 디버그 포트가 안 열림 → 디스코드 업데이터로 다시 실행', "Debug port didn't open with a direct launch → relaunching through Discord's updater", '直接起動ではデバッグポートが開かない → Discordのアップデーター経由で再起動'],
    ['통신', 'traffic', '通信'], ['내부', 'internal', '内部'], ['화면', 'screen', '画面'],
    ['통신 연결 대기', 'Waiting for connection', '通信接続待ち'],
    ['통신을 처음부터 받기 위해 디스코드 화면을 새로고침', 'Reloading Discord to read its connection from the start', '通信を最初から受け取るためDiscordを再読み込み'],
    ['통신 연결됨 — 모든 채널 감시 중', 'Connected to Discord traffic — watching all channels', '通信接続 — 全チャンネル監視中'],
    ['예비 감지: 연결됨', 'Backup detection: connected', '予備検知: 接続'],
    ['예비 감지: 실패 (통신 읽기로 감시 중이라 괜찮음)', 'Backup detection: failed (fine — traffic reading is active)', '予備検知: 失敗（通信読み取りで監視中なので問題なし）'],
    // 이유 (로그 괄호 안)
    ['서버 접속', 'server join', 'サーバー参加'],
    ['복귀', 'return', '復帰'],
    ['접속 실패', 'join failed', '参加失敗'],
    ['오토 팝핑 종료', 'auto popping finished', 'オートポッピング終了'],
    ['게임 접속 전', 'before game join', 'ゲーム参加前'],
    ['복귀 완료', 'return done', '復帰完了'],
  ];

  // 같은 글자라도 동작 편집기 안에서는 뜻이 다름 (대기 = 상태 '대기' 가 아니라 동작 '기다리기')
  const CTX = { step: { '대기': ['Wait', '待機'], '+ 대기': ['+ Wait', '+ 待機'] } };

  // ---------------------------------------------------------------- 패턴: [정규식, English, 日本語]
  // 문자열이면 $1… 자리에 (번역한) 묶음이 들어감 / 함수면 (번역한 묶음, 원래 묶음) 을 받음
  const opt = (v, f) => (v ? f(v) : '');
  const R = [
    // 매크로 탭 · 자동 낚시
    [/^Fish 클릭 \((\d+)번째\)$/, 'Clicking Fish (try $1)', 'Fishをクリック（$1回目）'],
    [/^낚시 결과: (\S+(?: \S+ \S+)?) · 성공 (\d+) \/ 쓰레기 (\d+) \/ 실패 (\d+)$/, 'Fishing result: $1 · Success $2 / Junk $3 / Fail $4', '釣り結果: $1・成功 $2 / ゴミ $3 / 失敗 $4'],
    [/^Fish 를 (\d+)번 눌러도 반응 없음 — 낚시 인벤토리 가득$/, 'Fish did not respond after $1 tries — fishing inventory full', 'Fishを$1回押しても反応なし — 釣りインベントリ満杯'],
    [/^입질이 ([\d.]+)초 동안 없음 — Exit 후 다시 던짐$/, 'No bite for $1s — pressing Exit and casting again', '$1秒間当たりなし — Exit後に再キャスト'],
    [/^릴링이 40초 넘게 끝나지 않음 — 다음으로$/, 'Reeling did not end after 40s — moving on', 'リールが40秒以上終わらない — 次へ'],
    [/^자동 낚시 안 함 — 매크로 기준 위치 설정 필요: (.+)$/, 'No auto fishing — Macro base position settings needed: $1', '自動釣りなし — マクロ基準位置の設定が必要: $1'],
    [/^자동 낚시 오류: (.+)$/, 'Auto fishing error: $1', '自動釣りのエラー: $1'],
    [/^(.+) 위치 저장$/, '$1 position saved', '$1の位置を保存'],
    [/^버튼: (.+)$/, 'Button: $1', 'ボタン: $1'],
    [/^릴링 바: 보임 · 내 위치 (\d+) · 구간 (.+)$/, 'Reel bar: visible · marker $1 · zone $2', 'リールバー: 見える・自分の位置 $1・区間 $2'],
    [/^릴링 바: (.+)$/, 'Reel bar: $1', 'リールバー: $1'],
    [/^결과창: (.+)$/, 'Result: $1', '結果: $1'],
    [/^레어 바이옴 자동 팝핑 — 자동 낚시가 자리를 비켜주지 않아 그냥 진행$/, 'Rare biome auto popping — auto fishing did not yield, continuing anyway', 'レアバイオーム自動ポッピング — 自動釣りが譲らないためそのまま進行'],
    [/^(.+) \(RapidOCR 없음\)$/, '$1 (no RapidOCR)', '$1（RapidOCRなし）'],
    // 메인 화면 기능별 버튼
    [/^(\d+)개 감지$/, '$1 detected', '$1件検知'],
    [/^기능 (\d+)개 켜짐$/, '$1 feature(s) on', '機能$1個オン'],
    [/^기능 (\d+)개$/, '$1 feature(s)', '機能$1個'],
    [/^작동 중: (.+)$/, 'Running: $1', '動作中: $1'],
    // 매크로 탭 · 레어 바이옴 자동 팝핑 (내 서버)
    [/^(\d+)개 지정 안 됨$/, '$1 not set', '$1個 未指定'],
    [/^오토 팝핑 설정 필요: (.+)$/, 'Auto popping setup needed: $1', 'オートポッピングの設定が必要: $1'],
    [/^레어 바이옴 자동 팝핑 (시작|테스트) — (\S+)( 템플릿| \(내 서버\))?$/, (g, r) => `Rare biome auto popping ${r[1] === '시작' ? 'started' : 'test'} — ${r[2]}${r[3] === ' 템플릿' ? ' template' : r[3] ? ' (your server)' : ''}`, (g, r) => `レアバイオーム自動ポッピング${r[1] === '시작' ? '開始' : 'テスト'} — ${r[2]}${r[3] === ' 템플릿' ? 'テンプレート' : r[3] ? '（自分のサーバー）' : ''}`],
    [/^레어 바이옴 자동 팝핑 완료 — (\S+) · (\d+)\/(\d+)개 사용$/, 'Rare biome auto popping done — $1 · used $2/$3', 'レアバイオーム自動ポッピング完了 — $1・$2/$3個使用'],
    [/^레어 바이옴 자동 팝핑 취소 — 매크로 기준 위치 설정 필요: (.+)$/, 'Rare biome auto popping canceled — Macro base position settings needed: $1', 'レアバイオーム自動ポッピング中止 — マクロ基準位置の設定が必要: $1'],
    [/^매크로 기준 위치 설정 필요: (.+)$/, 'Macro base position settings needed: $1', 'マクロ基準位置の設定が必要: $1'],
    [/^레어 바이옴 자동 팝핑 오류: (.+)$/, 'Rare biome auto popping error: $1', 'レアバイオーム自動ポッピングのエラー: $1'],
    [/^(\S+) 감지 — 스나이핑 접속이라 내 서버 팝핑 안 함$/, '$1 detected — sniping join, so no popping in your server', '$1を検知 — スナイプ参加のため自分のサーバーでのポッピングなし'],
    [/^(\S+) 감지 — 다른 매크로가 도는 중이라 내 서버 팝핑 안 함$/, '$1 detected — another macro is running, so no popping', '$1を検知 — 他のマクロが動作中のためポッピングなし'],
    [/^(\S+) 감지 — 매크로 탭 팝핑 바이옴에서 꺼져 있음$/, '$1 detected — turned off in the Macro tab popping biomes', '$1を検知 — マクロタブのポッピングバイオームでオフ'],
    [/^시작 전 대기 ([\d.]+)초$/, 'Waiting $1s before start', '開始前に$1秒待機'],
    [/^(\S+) 템플릿 적용$/, '$1 template applied', '$1テンプレートを適用'],
    [/^디버그 포트 (\d+) 를 다른 프로그램이 사용 중 → (\d+) 로 실행( \(.+\))?$/, (g, r) => `Debug port ${r[1]} is used by another program → using ${r[2]}${r[3] || ''}`, (g, r) => `デバッグポート${r[1]}は他のプログラムが使用中 → ${r[2]}で起動${r[3] || ''}`],
    [/^디버그 포트 (\d+) 가 열려 있지 않음$/, 'Debug port $1 is not open', 'デバッグポート$1が開いていません'],
    [/^디버그 포트 (\d+) 를 디스코드가 아닌 프로그램이 쓰고 있음 \((.+)\)$/, (g, r) => `Debug port ${r[1]} is used by a program other than Discord (${r[2]})`, (g, r) => `デバッグポート${r[1]}をDiscord以外のプログラムが使用中（${r[2]}）`],
    [/^디버그 포트 (\d+) 가 이 PC 밖\(네트워크\)에도 열려 있음$/, 'Debug port $1 is also open to the network (outside this PC)', 'デバッグポート$1がこのPCの外（ネットワーク）にも開いています'],
    [/^디스코드 보통 모드 재시작 실패: (.*)$/, (g, r) => `Failed to restart Discord in normal mode: ${r[1]}`, (g, r) => `Discordの通常モード再起動に失敗: ${r[1]}`],
    [/^디스코드에 연결됨 \((.*)\) — 통신 연결 대기 중…$/, (g, r) => `Connected to Discord (${r[1]}) — waiting for traffic…`, (g, r) => `Discordに接続（${r[1]}）— 通信待ち…`],
    [/^통신 압축 형식\((.*)\)을 풀 수 없음 — 예비 방식 사용$/, (g, r) => `Can't decompress traffic (${r[1]}) — using backup method`, (g, r) => `通信の圧縮形式（${r[1]}）を展開できません — 予備方式を使用`],
    [/^통신 읽기 오류: (.*)$/, (g, r) => `Traffic read error: ${r[1]}`, (g, r) => `通信読み取りエラー: ${r[1]}`],
    [/^↳ (통신|내부|화면) 방식은 ([\d.]+)초 늦게 도착$/, g => `↳ ${g[1]} method arrived ${g[2]}s later`, g => `↳ ${g[1]}方式は${g[2]}秒遅れて到着`],
    [/^PC 시계가 디스코드와 (\d+)초 차이 남 \((느림|빠름)\) — 윈도우 설정 → 시간 → '지금 동기화' 권장 \(감지는 보정해서 정상 작동\)$/,
      (g, r) => `PC clock is ${r[1]}s ${r[2] === '느림' ? 'behind' : 'ahead of'} Discord — sync it in Windows Settings → Time (detection compensates)`,
      (g, r) => `PCの時計がDiscordと${r[1]}秒ずれています（${r[2] === '느림' ? '遅れ' : '進み'}）— Windowsの設定 → 時刻で「今すぐ同期」推奨（検知は補正済み）`],
    [/^(\d+)초 뒤에 다시 보낼 수 있음$/, 'You can send again in $1s', '$1秒後に再送信できます'],
    [/^전송 실패: (.*)$/, 'Send failed: $1', '送信失敗: $1'],
    [/^(\d+)초 후 매크로 복귀를 실행합니다$/, 'Running macro return in $1s', '$1秒後にマクロ復帰を実行します'],
    [/^로블록스가 갑자기 꺼짐 — ([\d.]+)초 뒤 매크로 복귀 \(대기를 누르면 취소\)$/, 'Roblox closed unexpectedly — macro return in $1s (press Wait to cancel)', 'Robloxが突然終了 — $1秒後にマクロ復帰（待機で取り消し）'],
    [/^디스코드 응답 대기 중 \((\d+)초\)…$/, 'Waiting for Discord ($1s)…', 'Discordの応答を待機中（$1秒）…'],
    [/^스킵 \((\d+)번 더\)$/, 'Skip ($1 more)', 'スキップ（あと$1回）'],
    [/^남은 튜토리얼 (\d+)개$/, '$1 tutorial(s) left', '残りのチュートリアル $1件'],
    [/^서버 (\d+)개 전체$/, '$1 server(s)', 'サーバー$1個（全体）'],
    [/^채널 (\d+)개$/, '$1 channel(s)', 'チャンネル$1個'],
    [/^감시 대상: (.+)$/, 'Targets: $1', '監視対象: $1'],
    [/^모드: (.+)$/, 'Mode: $1', 'モード: $1'],
    [/^(.*?)\s+\(서버 ID임\)$/, (g, r) => `${r[1]}  (server ID)`, (g, r) => `${r[1]}  （サーバーID）`],
    [/^(.*?)\s+\(채널 ID임\)$/, (g, r) => `${r[1]}  (channel ID)`, (g, r) => `${r[1]}  （チャンネルID）`],
    [/^오토 팝핑 설정을 먼저 끝내야 시작 가능 \((\d+)개 남음\)$/, 'Finish the auto popping setup first ($1 left)', '先にオートポッピング設定を完了してください（残り$1個）'],
    [/^로블록스 화면에서 (.+) 클릭$/, 'Click $1 in Roblox', 'Robloxで$1をクリック'],
    [/^(.+) 위치 저장$/, '$1 position saved', '$1の位置を保存'],
    [/^(.+) 위치 먼저 지정$/, 'Set the $1 position first', '先に$1の位置を指定してください'],
    [/^(.+) 위치를 지정해주세요$/, 'Set the $1 position', '$1の位置を指定してください'],
    [/^이름: ([^·]*)$/, (g, r) => `Name: ${r[1]}`, (g, r) => `名前: ${r[1]}`],
    [/^개수: ([^·]*)$/, 'Count: $1', '個数: $1'],
    [/^기본 ([\d.]+)초$/, 'Default $1s', '初期値 $1秒'],
    [/^기본 ([\d.]+)$/, 'Default $1', '初期値 $1'],
    [/^(Inventory|Items|Search) 클릭 후$/, 'After clicking $1', '$1クリック後'],
    [/^포션 목록은 (.+) 탭$/, 'Potion list is in the $1 tab', 'ポーションリストは$1タブ'],
    [/^웹후크 (\d)$/, 'Webhook $1', 'Webhook $1'],
    [/^전송 완료 \((\d+\/\d+)\)$/, 'Sent ($1)', '送信完了（$1）'],
    [/^일부 실패 \((\d+\/\d+)\)$/, 'Partly failed ($1)', '一部失敗（$1）'],
    [/^현재 바이옴 (.+)$/, 'Current biome $1', '現在のバイオーム $1'],
    [/^최근 (\d+)개$/, 'Last $1', '直近$1件'],
    [/^키 조합: (.+)$/, 'Key combo: $1', 'キーの組み合わせ: $1'],
    [/^프로그램 선택: (.+)$/, (g, r) => `Program selected: ${r[1]}`, (g, r) => `プログラム選択: ${r[1]}`],
    [/^창 선택: (.+)$/, (g, r) => `Window selected: ${r[1]}`, (g, r) => `ウィンドウ選択: ${r[1]}`],
    // 튜토리얼 (이름이 들어가는 단계)
    [/^(.+?)[을를] 눌러주세요$/, 'Click $1', '$1をクリックしてください'],
    [/^왼쪽 목록에서 강조된 <b>(.+?)<\/b>[을를] 눌러주세요\.$/, 'Click the highlighted <b>$1</b> in the left list.', '左のリストで強調された<b>$1</b>をクリックしてください。'],
    [/^강조된 <b>(.+?)<\/b> 버튼을 직접 눌러주세요\.$/, 'Click the highlighted <b>$1</b> button yourself.', '強調された<b>$1</b>ボタンを直接クリックしてください。'],
    [/^<b>위치 지정<\/b>을 누른 뒤 로블록스 화면에서 <b>(.+?)<\/b>(?: \((.+?)\))?을 한 번 클릭해주세요\.$/,
      g => `Click <b>Set position</b>, then click <b>${g[1]}</b>${opt(g[2], v => ` (${v})`)} once in Roblox.`,
      g => `<b>位置を指定</b>を押してから、Robloxで<b>${g[1]}</b>${opt(g[2], v => `（${v}）`)}を一度クリックしてください。`],
    // 프로그램 쪽 로그 · 오류
    [/^이름 (\d+)개를 디스코드에서 가져옴$/, 'Fetched $1 name(s) from Discord', 'Discordから名前を$1件取得'],
    [/^실행 실패: (.*)$/, 'Launch failed: $1', '起動失敗: $1'],
    [/^OCR 테스트: '(.*)' → 이름 '(.*)' · 개수 (\S*)(?: · '(.*)' 일치율 ([\d.]+)%)?$/,
      (g, r) => `OCR test: '${r[1]}' → name '${r[2]}' · count ${r[3]}` + opt(r[4], v => ` · '${v}' match ${r[5]}%`),
      (g, r) => `OCRテスト: '${r[1]}' → 名前 '${r[2]}'・個数 ${r[3]}` + opt(r[4], v => `・'${v}' 一致率 ${r[5]}%`)],
    [/^OCR 테스트: (.*)$/, 'OCR test: $1', 'OCRテスト: $1'],
    [/^설정 필요: (.+)$/, 'Setup needed: $1', '設定が必要: $1'],
    [/^OCR 패키지 없음: (.*)$/, 'OCR package missing: $1', 'OCRパッケージなし: $1'],
    [/^시작 안 됨 — 오토 팝핑 설정 필요: (.+)$/, 'Not started — auto popping setup needed: $1', '開始できません — オートポッピング設定が必要: $1'],
    [/^OCR 엔진: (.+)$/, 'OCR engine: $1', 'OCRエンジン: $1'],
    [/^OCR 실패 \(상태 (.+)\)$/, 'OCR failed (status $1)', 'OCR失敗（状態 $1）'],
    [/^config\.json 읽기 실패 \(기본값 사용\): (.*)$/, 'Failed to read config.json (using defaults): $1', 'config.jsonの読み込み失敗（初期値を使用）: $1'],
    [/^디스코드를 찾을 수 없음: (.*?)\s+\(기타 설정에서 디스코드 종류 확인 필요\)$/, (g, r) => `Discord not found: ${r[1]}  (check Discord version in Other settings)`, (g, r) => `Discordが見つかりません: ${r[1]} （その他の設定でDiscordの種類を確認してください）`],
    [/^메인 창 대기 중… 현재 창: (.*)$/, (g, r) => `Waiting for the main window… current: ${r[1]}`, (g, r) => `メインウィンドウ待機中… 現在: ${r[1]}`],
    [/^([\d.]+)초 뒤 접속 \(사람처럼 잠깐 대기\)$/, 'Joining in $1s (short human-like wait)', '$1秒後に参加（人のように少し待機）'],
    [/^웹브라우저로 링크 열기( \(안정화\))? → (.+)$/, (g, r) => `Opening link in web browser${opt(r[1], () => ' (stable)')} → ${r[2]}`, (g, r) => `Webブラウザでリンクを開く${opt(r[1], () => '（安定）')} → ${r[2]}`],
    [/^딥링크 실행( \(안정화\))? → (.+)$/, (g, r) => `Deep link opened${opt(r[1], () => ' (stable)')} → ${r[2]}`, (g, r) => `ディープリンク起動${opt(r[1], () => '（安定）')} → ${r[2]}`],
    [/^딥링크 실행 실패 \((.*)\) → 브라우저로 열기$/, 'Deep link failed ($1) → opening in browser', 'ディープリンク失敗（$1）→ ブラウザで開く'],
    [/^링크 열기 실패: (.*)$/, 'Failed to open link: $1', 'リンクを開けません: $1'],
    [/^복귀 링크 실행 → (.+)$/, (g, r) => `Return link opened → ${r[1]}`, (g, r) => `復帰リンク起動 → ${r[1]}`],
    [/^\[([^\]]+)\] (.*?)\s+→ (\S+)(?:\s+\(([\d.]+)초 · (통신|내부|화면|\S+)\))?$/,
      (g, r) => `[${g[1]}] ${r[2]}  → ${r[3]}` + opt(r[4], v => `  (${v}s · ${{ 통신: 'traffic', 내부: 'internal', 화면: 'screen' }[r[5]] || r[5]})`),
      (g, r) => `[${g[1]}] ${r[2]}  → ${r[3]}` + opt(r[4], v => `  (${v}秒 · ${{ 통신: '通信', 내부: '内部', 화면: '画面' }[r[5]] || r[5]})`)],
    [/^디스코드에 연결됨 \((.*)\) — Flux 연결 대기 중…$/, (g, r) => `Connected to Discord (${r[1]}) — waiting for Flux…`, (g, r) => `Discordに接続（${r[1]}）— Flux接続待ち…`],
    [/^\[진단\] (.*)$/, (g, r) => `[Diag] ${r[1]}`, (g, r) => `[診断] ${r[1]}`],
    [/^웹후크 전송 실패: (.*)$/, 'Webhook send failed: $1', 'Webhook送信失敗: $1'],
    [/^닉네임 불일치 — 로그의 닉네임 '(.*)' ≠ 입력한 '(.*)' · 감지 안 함$/, (g, r) => `Username mismatch — log has '${r[1]}' ≠ entered '${r[2]}' · not detecting`, (g, r) => `ユーザー名不一致 — ログの名前 '${r[1]}' ≠ 入力 '${r[2]}'・検出しません`],
    [/^계정 확인됨: (.*)$/, (g, r) => `Account confirmed: ${r[1]}`, (g, r) => `アカウント確認: ${r[1]}`],
    [/^바이옴: (.+) → (.+)$/, 'Biome: $1 → $2', 'バイオーム: $1 → $2'],
    [/^바이옴: (.+)$/, 'Biome: $1', 'バイオーム: $1'],
    [/^오토 팝핑 취소 — 설정 필요: (.+)$/, 'Auto popping cancelled — setup needed: $1', 'オートポッピング中止 — 設定が必要: $1'],
    [/^오토 팝핑 테스트 — (.+) 템플릿$/, 'Auto popping test — $1 template', 'オートポッピングテスト — $1テンプレート'],
    [/^입장 후 대기 ([\d.]+)초$/, 'Waiting $1s after entering', '参加後の待機 $1秒'],
    [/^오토 팝핑 안 함 — (.+) 는 팝핑 바이옴 설정에서 꺼져 있음$/, 'No auto popping — $1 is off in Popping biomes', 'オートポッピングなし — $1はポッピング対象バイオームでオフ'],
    [/^오토 팝핑 안 함 — 현재 바이옴 (.+) \(템플릿 없음\)$/, 'No auto popping — current biome $1 (no template)', 'オートポッピングなし — 現在のバイオーム $1（テンプレートなし）'],
    [/^오토 팝핑 시작 — (.+)$/, 'Auto popping started — $1', 'オートポッピング開始 — $1'],
    [/^(.+) 템플릿에 포션이 없음$/, 'No potions in the $1 template', '$1テンプレートにポーションがありません'],
    [/^오토 팝핑 완료 — (.+) · (\d+)\/(\d+)개 사용$/, 'Auto popping done — $1 · used $2/$3', 'オートポッピング完了 — $1・$2/$3個使用'],
    [/^(.+) 끝날 때까지 대기$/, 'Waiting for $1 to end', '$1の終了を待機中'],
    [/^바이옴 종료 \((.+) → (.+)\) — 오토 팝핑 종료$/, 'Biome ended ($1 → $2) — auto popping finished', 'バイオーム終了（$1 → $2）— オートポッピング終了'],
    [/^(.+) 검색( \(재시도\))?$/, (g, r) => `Searching ${r[1]}${opt(r[2], () => ' (retry)')}`, (g, r) => `${r[1]}を検索${opt(r[2], () => '（再試行）')}`],
    [/^OCR: '(.*)' · (.+) 일치율 ([\d.]+)%$/, (g, r) => `OCR: '${r[1]}' · ${r[2]} match ${r[3]}%`, (g, r) => `OCR: '${r[1]}'・${r[2]} 一致率 ${r[3]}%`],
    [/^(.+) — 일치율 ([\d.]+)% 미만, 스킵$/, (g, r) => `${r[1]} — below ${r[2]}% match, skipped`, (g, r) => `${r[1]} — 一致率${r[2]}%未満、スキップ`],
    [/^(.+) — 개수를 읽지 못해 스킵$/, (g, r) => `${r[1]} — couldn't read the count, skipped`, (g, r) => `${r[1]} — 個数を読めずスキップ`],
    [/^(.+) — 보유 (\d+)개 \(필요 (\d+)개\), 스킵$/, (g, r) => `${r[1]} — have ${r[2]} (need ${r[3]}), skipped`, (g, r) => `${r[1]} — 所持${r[2]}個（必要${r[3]}個）、スキップ`],
    [/^(.+) (\d+)개 사용$/, (g, r) => `${r[1]} ×${r[2]} used`, (g, r) => `${r[1]}を${r[2]}個使用`],
    [/^(.+) 위치가 지정되지 않아 클릭 안 함 \(오토 팝핑 매크로 설정에서 지정\)$/, 'Not clicking — $1 position not set (set it in Auto popping macro settings)', '$1の位置が未指定のためクリックしません（オートポッピングマクロ設定で指定）'],
    [/^Play 버튼 자동 클릭 시작(?: \((.+)\))?$/, g => `Play auto-click started${opt(g[1], v => ` (${v})`)}`, g => `Playボタン自動クリック開始${opt(g[1], v => `（${v}）`)}`],
    [/^로블록스 창 감지 \((.+)\) — ([\d.]+)초 뒤 Play \/ Click to skip 클릭$/, 'Roblox window found ($1) — clicking Play / Click to skip in $2s', 'Robloxウィンドウ検出（$1）— $2秒後にPlay / Click to skipをクリック'],
    [/^로딩 대기 ([\d.]+)초$/, 'Loading wait $1s', '読み込み待機 $1秒'],
    [/^(Play|Click to skip) 클릭 — 화면 좌표 \((.+)\) · 로블록스 창 (\S+) · (.+)$/, 'Clicked $1 — screen ($2) · Roblox window $3 · $4', '$1をクリック — 画面座標（$2）・Robloxウィンドウ $3・$4'],
    [/^(Play|Click to skip) 클릭 \((\d+)번째\)$/, '$1 click (#$2)', '$1クリック（$2回目）'],
    [/^게임 입장 확인 — Play 클릭 종료 \(클릭 (\d+)번\)$/, 'In game — Play clicking finished ($1 clicks)', 'ゲーム参加を確認 — Playクリック終了（$1回）'],
    [/^(\d+)초 동안 게임 입장이 확인되지 않아 Play 클릭 중지$/, 'Not in game after $1s — Play clicking stopped', '$1秒間ゲーム参加を確認できずPlayクリックを停止'],
    [/^매크로 복귀 시작(?: \((.+)\))? — 로블록스 종료$/, g => `Macro return started${opt(g[1], v => ` (${v})`)} — closing Roblox`, g => `マクロ復帰開始${opt(g[1], v => `（${v}）`)} — Robloxを終了`],
    [/^모르는 키: (.*)$/, 'Unknown key: $1', '不明なキー: $1'],
    [/^(.+?) 시작(?: \((.+)\))? — (\d+)개 · 정지: F7$/, g => `${g[1]} started${opt(g[2], v => ` (${v})`)} — ${g[3]} action(s) · Stop: F7`, g => `${g[1]}開始${opt(g[2], v => `（${v}）`)} — ${g[3]}個・停止: F7`],
    [/^([\d.]+)초 뒤 시작$/, 'Starting in $1s', '$1秒後に開始'],
    [/^'(.+)' 창을 찾을 수 없음$/, (g, r) => `'${r[1]}' window not found`, (g, r) => `「${r[1]}」ウィンドウが見つかりません`],
    [/^프로그램 강제 종료: (.+?)( \(실행 중 아님\))?$/, (g, r) => `Force-closed: ${r[1]}${opt(r[2], () => ' (not running)')}`, (g, r) => `強制終了: ${r[1]}${opt(r[2], () => '（起動していません）')}`],
    [/^모르는 동작: (.*)$/, 'Unknown action: $1', '不明な動作: $1'],
    [/^프로그램 실행: (.+)$/, (g, r) => `Program launched: ${r[1]}`, (g, r) => `プログラム起動: ${r[1]}`],
    [/^(\d+)\. (.+)$/, (g, r) => `${r[1]}. ${ctx('step', r[2]) ?? g[2]}`, (g, r) => `${r[1]}. ${ctx('step', r[2]) ?? g[2]}`],
    [/^오류: (.*)$/, 'Error: $1', 'エラー: $1'],
    [/^설정 저장 실패: (.*)$/, 'Failed to save settings: $1', '設定の保存に失敗: $1'],
    [/^권한이 없어 종료 못 함: (.+?) — 관리자 권한으로 실행된 프로그램이면 Acrux 도 관리자 권한으로 실행 필요$/, (g, r) => `No permission to end: ${r[1]} — if it runs as administrator, run Acrux as administrator too`, (g, r) => `権限がなく終了できません: ${r[1]} — 管理者として実行されている場合はAcruxも管理者として実行してください`],
    [/^프로세스 종료 실패: (.*)$/, 'Failed to end process: $1', 'プロセス終了失敗: $1'],
    [/^(\w+(?:Error|Exception)): (.*)$/, (g, r) => `${r[1]}: ${g[2]}`, (g, r) => `${r[1]}: ${g[2]}`],
    [/^([^·]+?) 오류: (.*)$/, '$1 error: $2', '$1エラー: $2'],
  ];
  // 일반 패턴 — 글자를 ' · ' 등으로 나눠 본 뒤에도 못 찾았을 때만 (긴 문장 전체에 먼저 걸리지 않게)
  const G = [
    [/^(.+) 완료$/, '$1 done', '$1完了'],
    [/^(.+) 정지$/, '$1 stopped', '$1停止'],
    [/^(.+) 시작$/, '$1 started', '$1開始'],
    [/^(.+) 버튼$/, '$1 button', '$1ボタン'],
    [/^(.+) 위치$/, '$1 position', '$1の位置'],
    [/^예: (.+)$/, (g, r) => `e.g. ${r[1]}`, (g, r) => `例: ${r[1]}`],
    [/^\+ (.+)$/, '+ $1', '+ $1'],
    [/^(.+) :$/, '$1 :', '$1 :'],
  ];

  // ---------------------------------------------------------------- 번역
  const norm = s => s.replace(/\s+/g, ' ').trim();
  const LI = { en: 1, ja: 2 };
  const DICT = new Map(E.map(e => [norm(e[0]), e]));
  function ctx(name, s) {
    const e = CTX[name]?.[norm(s)];
    return e ? e[LI[lang] - 1] : null;
  }
  // 나누는 순서: ' · ' 로 먼저 (가장 큰 단위), 안 되면 ' — ', ' → ', ' + '
  const SEPS = [/(\s+·\s+)/, /(\s+—\s+)/, /(\s+→\s+)/, /(\s+\+\s+)/];

  function lookup(key, depth, generic = false) {
    if (!generic) {
      const e = DICT.get(key);
      if (e) return e[LI[lang]];
    }
    for (const p of generic ? G : R) {
      const m = key.match(p[0]);
      if (!m) continue;
      const out = p[LI[lang]];
      const g = m.map((v, n) => (n === 0 || v === undefined ? v : tr(v, depth + 1, true)));
      if (typeof out === 'function') return out(g, m);
      return out.replace(/\$(\d)/g, (_, n) => g[n] ?? '');
    }
    return null;
  }

  // list: 묶음 안이 'A, B' 목록이면 하나씩
  function tr(s, depth = 0, list = false) {
    if (lang === 'ko' || s == null) return s;
    s = String(s);
    if (!HANGUL.test(s) || depth > 5) return s;
    const lead = s.match(/^\s*/)[0], tail = s.match(/\s*$/)[0];
    const core = s.slice(lead.length, s.length - tail.length);
    let out = lookup(norm(core), depth);
    if (out == null && list && core.includes(', ')) {
      out = core.split(', ').map(x => tr(x, depth + 1)).join(', ');
    }
    for (const sep of SEPS) {
      if (out != null) break;
      const parts = core.split(sep);
      if (parts.length > 1) out = parts.map((x, n) => (n % 2 ? x : tr(x, depth + 1, true))).join('');
    }
    if (out == null) out = lookup(norm(core), depth, true);
    if (out == null) { missing.add(norm(core)); out = core; }
    return lead + out + tail;
  }
  const missing = new Set();

  // ---------------------------------------------------------------- 화면에 적용
  const textRec = new WeakMap();      // 글자 조각 → { src: 원래 한국어, out: 마지막으로 넣은 글자 }
  const richRec = new WeakMap();      // <p>/<li> → { src: 원래 HTML, out }
  const attrRec = new WeakMap();      // 요소 → { placeholder: {src, out}, title: {...} }
  const ATTRS = ['placeholder', 'title'];
  const SKIP = 'script, style, textarea, [data-notr]';
  const RICH = 'p, li';

  function doText(node) {
    const el = node.parentElement;
    if (!el || el.closest(SKIP)) return;
    const v = node.nodeValue;
    let r = textRec.get(node);
    if (!r || v !== r.out) r = { src: v };
    if (!r.src.trim()) return;
    const c = el.closest('[data-tctx]')?.dataset.tctx;
    const out = lang === 'ko' ? r.src : (c && ctx(c, r.src)) ?? tr(r.src);
    r.out = out;
    textRec.set(node, r);
    if (v !== out) node.nodeValue = out;
  }

  function doRich(el) {
    if (el.closest(SKIP)) return;
    const h = el.innerHTML;
    let r = richRec.get(el);
    if (!r || h !== r.out) r = { src: h };
    richRec.set(el, r);
    if (lang === 'ko') {
      if (h !== r.src) el.innerHTML = r.src;
    } else if (el.children.length && HANGUL.test(r.src)) {
      const out = lookup(norm(r.src), 0);
      if (out != null) { if (h !== out) el.innerHTML = out; }
      else {
        if (h !== r.src) el.innerHTML = r.src;
        for (const n of textNodes(el)) doText(n);
      }
    } else {
      if (h !== r.src) el.innerHTML = r.src;
      for (const n of textNodes(el)) doText(n);
    }
    r.out = el.innerHTML;
  }

  function doAttrs(el) {
    if (el.closest(SKIP)) return;
    for (const a of ATTRS) {
      if (!el.hasAttribute(a)) continue;
      const v = el.getAttribute(a);
      const all = attrRec.get(el) || {};
      let r = all[a];
      if (!r || v !== r.out) r = { src: v };
      const out = lang === 'ko' ? r.src : tr(r.src);
      r.out = out; all[a] = r; attrRec.set(el, all);
      if (v !== out) el.setAttribute(a, out);
    }
  }

  function textNodes(root) {
    const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const out = [];
    while (w.nextNode()) out.push(w.currentNode);
    return out;
  }

  // root 아래 전부 (지금 바로)
  function apply(root = document.body) {
    pause(() => {
      const rich = new Set();
      const els = root.nodeType === 1 ? [root, ...root.querySelectorAll('*')] : [];
      for (const el of els) {
        if (el.matches(RICH) && !el.parentElement?.closest(RICH)) rich.add(el);
        doAttrs(el);
      }
      for (const n of textNodes(root)) {
        const r = n.parentElement?.closest(RICH);
        if (r) rich.add(r); else doText(n);
      }
      rich.forEach(doRich);
    });
  }

  let observer = null;
  // 번역하면서 바꾼 글자는 다시 감지하지 않게 잠깐 끔 (그 전에 쌓인 변화는 따로 챙겨서 처리)
  function pause(fn) {
    const before = observer ? observer.takeRecords() : [];
    observer?.disconnect();
    try { fn(); } finally { observer?.observe(document.body, OBS); }
    if (before.length) onMutations(before);
  }
  const OBS = { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS };
  function onMutations(list) {
    const rich = new Set(), texts = new Set(), elems = new Set();
    for (const m of list) {
      if (m.type === 'characterData') texts.add(m.target);
      else if (m.type === 'attributes') elems.add(m.target);
      else m.addedNodes.forEach(n => {
        if (n.nodeType === 3) texts.add(n);
        else if (n.nodeType === 1) { elems.add(n); n.querySelectorAll('*').forEach(x => elems.add(x)); textNodes(n).forEach(x => texts.add(x)); }
      });
    }
    pause(() => {
      for (const el of elems) {
        if (!el.isConnected) continue;
        doAttrs(el);
        const r = el.closest(RICH);
        if (r) rich.add(r);
      }
      for (const n of texts) {
        if (!n.isConnected) continue;
        const r = n.parentElement?.closest(RICH);
        if (r) rich.add(r); else doText(n);
      }
      rich.forEach(r => { if (r.isConnected) doRich(r); });
    });
  }

  // ---------------------------------------------------------------- 언어 선택
  const listeners = [];
  function set(l) {
    if (!LANGS[l]) l = 'ko';
    lang = l;
    document.documentElement.lang = l;
    apply(document.body);
    listeners.forEach(f => f(l));
  }
  // 처음 실행: 윈도우 언어로 (한국어 · 일본어 · 나머지는 영어)
  function detect() {
    const n = (navigator.language || 'ko').toLowerCase();
    return n.startsWith('ko') ? 'ko' : n.startsWith('ja') ? 'ja' : 'en';
  }
  function init() {
    if (observer) return;
    observer = new MutationObserver(onMutations);
    observer.observe(document.body, OBS);
  }
  // 원래(한국어) 글자 — 화면 글자를 읽어서 다시 쓸 때 (번역된 글자가 원본으로 굳지 않게)
  function source(el) {
    const n = [...el.childNodes].find(x => x.nodeType === 3 && x.nodeValue.trim());
    return (n && textRec.get(n)?.out === n.nodeValue ? textRec.get(n).src : n?.nodeValue) ?? el.textContent;
  }

  return {
    LANGS, init, set, detect, apply, source, t: s => tr(s),
    get lang() { return lang; },
    onChange: f => listeners.push(f),
    missing: () => [...missing],
  };
})();
