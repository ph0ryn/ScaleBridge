# ScaleBridge 仕様

## 参照

BLEプロトコル仕様は [`./protocol.md`](./protocol.md) を正とします。
この仕様書ではアプリ構成、常駐方式、保存方式、UI境界だけを扱います。

## 目的

ScaleBridgeは、eufy系BLE体重計の測定値をローカルで取得し、ユーザーがアプリ画面を
開いていない間もバックグラウンドで検出、接続、保存できるmacOS向け
常駐アプリを作ります。

主目的:

- ログイン時に自動起動する。
- 常駐中のCPU、メモリ、UIコストを小さく保つ。
- 体重計が起きたタイミングでBLE接続する。
- 測定値と生パケットをSQLiteへ保存する。
- 画面はメニューバーから開いた時だけ生成する。
- 調査と障害対応のため、同じcoreを使うCLIも残す。

## スコープ

対象:

- Rust製のBLE取得core。
- coreを使ったstdio確認用CLI。
- Tauri製macOSメニューバー常駐アプリ。
- SQLiteによるローカル保存。
- T9120系の測定値取得。
- 未対応機種の検出ログと生パケット保存。

対象外:

- クラウドログイン。
- クラウド同期。
- 公式アカウントやサーバAPI連携。
- 体組成の完全計算。
- スマートフォンアプリ互換UI。
- 常時WebViewを起動し続ける実装。

## 要件

### R1: 常駐起動

アプリはmacOSログイン時に自動起動できること。
自動起動の有効/無効はユーザーが切り替えられること。

実装方針:

- Tauriのautostart機能を使う。
- macOSではLaunchAgentとして登録する。
- 手動起動時とログイン起動時で同じbackendを使う。

### R2: 低コスト常駐

通常時はBLE監視と最小限の状態保持だけを行い、WebViewを起動し続けないこと。

実装方針:

- 起動時はtray/menu bar iconだけを生成する。
- window設定は起動時自動作成しない。
- メニューバー操作時に初めてWebView windowを作る。
- windowを閉じたらhideではなくdestroyを基本にする。
- Rust backendはwindowの有無に関係なく動作する。

### R3: BLE取得

BLEのscan、接続、notify購読、write、parseはRust coreで行うこと。
プロトコル処理は [`./protocol.md`](./protocol.md) に従うこと。

実装方針:

- `T9120`系を最初の対応対象にする。
- `FFF0`だけでは機種確定しない。
- 未知の機種はraw loggingを優先する。
- 接続失敗、timeout、切断後はscanへ戻る。

### R4: ローカル保存

測定値、raw packet、deviceをSQLiteへ保存すること。
UIはDBを直接操作せず、Tauri command経由でbackendから取得すること。

実装方針:

- DB accessはRust backendに閉じる。
- frontendへSQL pluginを公開しない。
- raw packetを保存し、後からparser改善や他機種解析に使えるようにする。

### R5: CLI

調査とデバッグ用に、同じcoreを使うCLIを提供すること。

必要な挙動:

- `watch`でscanを開始する。
- 測定値、接続状態、raw packetをstdioへ出す。
- Tauri appがなくても単体で動く。

### R6: UI

UIは保存済みデータと現在状態の表示に集中すること。

必要な表示:

- 最新測定値。
- 測定履歴。
- 接続状態。
- 検出中device。
- raw packet/logの簡易表示。
- autostart設定。

UIが開いている時だけ、backend eventを購読してリアルタイム更新すること。

## アーキテクチャ

```text
src
  frontend application
  invokes backend commands
  subscribes to backend events while open

public
  static frontend assets

src-tauri
  Tauri app shell
  tray/menu
  autostart
  backend runtime
  Tauri commands
  lazy window creation

crates/scalebridge-core
  BLE scan/connect
  GATT I/O
  protocol parsing
  device profiles
  measurement events

crates/scalebridge-storage
  SQLite schema
  repository layer
  migrations

crates/scalebridge-cli
  stdio debugging
  watch command
  raw packet dump
```

依存方向:

```text
src -> Tauri commands -> src-tauri -> core -> protocol
                           src-tauri -> storage
CLI -------------------------------> core
CLI -------------------------------> storage, optional
```

禁止する依存:

- `core`からTauriへ依存しない。
- `core`からfrontendへ依存しない。
- `src`からSQLiteを直接触らない。
- `src`へBLE処理を持たせない。

## リポジトリ構造

Tauri単体アプリとして扱いやすい標準構成を基本にします。frontendはrootの
`src/`に置き、Tauri/Rust app shellは`src-tauri/`に置きます。
BLEとstorageはTauriに閉じ込めず、rootのRust workspace内の共有crateにします。

```text
.
├── Cargo.toml
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json
├── public/
├── src/
│   ├── app/
│   ├── components/
│   ├── features/
│   ├── lib/
│   │   └── tauri.ts
│   ├── stores/
│   ├── styles/
│   └── main.tsx
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands/
│       ├── services/
│       ├── state/
│       └── error.rs
├── crates/
│   ├── scalebridge-core/
│   ├── scalebridge-storage/
│   └── scalebridge-cli/
└── docs/
    ├── protocol.md
    └── spec.md
```

rootの`Cargo.toml`はRust workspaceとして扱います。

```toml
[workspace]
members = [
  "src-tauri",
  "crates/scalebridge-core",
  "crates/scalebridge-storage",
  "crates/scalebridge-cli",
]
resolver = "2"
```

`src-tauri/Cargo.toml`は共有crateをpath dependencyとして参照します。

```toml
[dependencies]
scalebridge-core = { path = "../crates/scalebridge-core" }
scalebridge-storage = { path = "../crates/scalebridge-storage" }
```

Tauri configはViteのdev serverとbuild outputを参照します。

```json
{
  "build": {
    "beforeDevCommand": "pnpm dev",
    "beforeBuildCommand": "pnpm build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  }
}
```

## ランタイム設計

### ログイン起動

```text
macOS login
  -> Tauri app starts
  -> tray icon is created
  -> backend runtime starts
  -> SQLite is opened
  -> BLE watcher starts
  -> no WebView window is created
```

### 測定時

```text
scale wakes up
  -> BLE watcher discovers candidate
  -> core connects
  -> core subscribes notify
  -> core sends init commands
  -> core receives packets
  -> parser emits measurement/raw events
  -> storage writes records
  -> UI receives event only if window is open
```

### UI表示

```text
menu bar click
  -> create WebView window if absent
  -> frontend invokes get_current_status()
  -> frontend invokes list_recent_measurements() for stable results
  -> frontend subscribes to live events
```

### UI終了

```text
window close
  -> frontend unsubscribes live events
  -> WebView window is destroyed
  -> backend BLE watcher continues
```

## データモデル

初期schema:

```sql
CREATE TABLE devices (
  id INTEGER PRIMARY KEY,
  model TEXT,
  name TEXT,
  address TEXT,
  service_uuids_json TEXT NOT NULL DEFAULT '[]',
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL
);

CREATE TABLE raw_packets (
  id INTEGER PRIMARY KEY,
  device_id INTEGER,
  seen_at TEXT NOT NULL,
  direction TEXT NOT NULL,
  characteristic_uuid TEXT,
  hex TEXT NOT NULL,
  parser TEXT,
  parsed_json TEXT,
  FOREIGN KEY(device_id) REFERENCES devices(id)
);

CREATE TABLE measurements (
  id INTEGER PRIMARY KEY,
  device_id INTEGER,
  measured_at TEXT NOT NULL,
  weight_kg REAL,
  impedance INTEGER,
  encrypted_impedance INTEGER,
  stable INTEGER NOT NULL,
  raw_packet_id INTEGER,
  FOREIGN KEY(device_id) REFERENCES devices(id),
  FOREIGN KEY(raw_packet_id) REFERENCES raw_packets(id)
);

```

保存方針:

- 測定値は正規化して`measurements`へ保存する。
- `dynamic`は測定中のlive状態として保存するが、測定結果UI/APIは`stable`のみを返す。
- 受信/送信したBLE packetは`raw_packets`へ保存する。
- 未知packetも捨てない。
- parse失敗もraw packetとして保存し、後からparser改善に使えるようにする。
- stable packetの重複はbackendでdebounceする。

## Backend API

Tauri commands:

```text
get_current_status() -> AppStatus
list_recent_measurements(limit: u32) -> Vec<Measurement> stable results
list_devices() -> Vec<Device>
set_autostart_enabled(enabled: bool) -> AutostartStatus
get_autostart_status() -> AutostartStatus
start_watcher() -> WatcherStatus
stop_watcher() -> WatcherStatus
```

Public WatcherStatus values:

```text
starting
watching
connecting
connected
subscribed
stopping
stopped
```

Scanning and rescan-wait phases are both exposed as `watching`.

Backend events:

```text
watcher://status-changed
watcher://device-seen
watcher://measurement-created
watcher://packet-received
watcher://error
```

eventはUIが開いている時だけ購読される前提です。
購読者がいない時もbackend処理とDB保存は継続します。

## Core API

core crateはTauriに依存しないAPIにします。

```text
ScaleWatcher::run(config, event_sink)
ScaleWatcher::stop()
ProtocolProfile::detect(advertisement, services)
ProtocolProfile::connect(peripheral)
PacketParser::parse_notification(bytes)
```

coreがemitするevent:

```text
DeviceSeen
Connected
Disconnected
RawPacket
Measurement
ParseWarning
TransportError
```

## 機種対応方針

初期対応:

- `T9120`実測済みprofile。

ログのみ対応:

- `T9140 V1/V2/V3`候補。
- `T9148/T9149`候補。
- `T9150/T9130`候補。
- `FFF0 unknown`。

未検証機種では、測定値として保存する前に次を満たす必要があります。

- service/characteristic構成が記録されている。
- raw packetが保存されている。
- parser仕様が`./protocol.md`に追加されている。
- 実測でweight値の妥当性を確認している。

## 権限とプライバシー

- Bluetooth権限はmacOSアプリ本体に付与される想定にする。
- 測定値とraw packetはローカル保存のみ。
- 初期仕様では外部送信しない。
- DBファイルの場所はアプリデータディレクトリ配下に置く。
- export機能を入れる場合も明示操作に限定する。

## ログ

通常ログ:

- 起動/終了。
- watcher開始/停止。
- device検出。
- 接続/切断。
- 測定値保存。
- parse警告。
- BLEエラー。

raw packetログ:

- DBへ保存する。
- CLIではstdioへ出す。
- UIでは直近分だけ表示する。

## 検証

必須確認:

- CLI単体でT9120を検出できる。
- CLI単体でT9120のstable weightをstdio出力できる。
- Tauri常駐時にwindow未生成でBLE監視できる。
- windowを開くとDBから直近測定値を表示できる。
- windowを閉じるとWebViewが破棄される。
- windowが閉じていても測定値がDBへ保存される。
- ログイン時自動起動を有効/無効にできる。
- 未知packetが破棄されず保存される。

手動確認:

- 体重計に乗る。
- 測定完了まで待つ。
- windowを開いて測定値が表示されることを確認する。
- DBに`measurements`と`raw_packets`が残ることを確認する。

## 未確定事項

- DB crateは`sqlx`と`rusqlite`のどちらにするか。
- Tauri windowを毎回destroyするか、短時間だけhide再利用するか。
- autostart設定UIの配置。
- raw packet保持期間。
- DB export形式。
- 複数体重計が近くにある場合のdevice選択UI。
- T9140以降の実測対応順。

## 初期実装順

1. Rust workspaceを作る。
2. `scalebridge-core`を作る。
3. `scalebridge-cli watch`を作る。
4. SQLite storageを作る。
5. CLIからDB保存できるようにする。
6. Tauri app shellを作る。
7. 起動時window非生成、trayのみ生成にする。
8. Tauri backendからcore watcherを起動する。
9. Tauri commandsでDB内容を返す。
10. frontendで最新状態と履歴を表示する。
11. autostartを実装する。
