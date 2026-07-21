# TypingEffect v0.9.11 — Source Guide

TypingEffectは、AviUtl2向けの日本語・中文対応IMEタイピングアニメーションです。この文書ではソース構成、ビルド、検査、配布パッケージの作成方法を説明します。利用者向けの操作方法は [`README.txt`](README.txt) を参照してください。

## アーキテクチャ

TypingEffectはLua描画スクリプトとRustスクリプトモジュールの2層で構成されています。

- `TypingEffect.obj2`
  - 入力イベントのタイムライン生成
  - テキスト書式・制御文字の分類と描画文字列への適用
  - `<r>` / `<w>` 時間制御文字のイベント時間への変換
  - 座標指定、スクリプト、クリア制御文字の除外
  - テキスト、未確定下線、カーソルの描画
  - フォント字面のアルファ領域に基づくカーソルメトリクス計測
- `TypingEffect.mod2`
  - 日本語・中文の形態素解析
  - 読みからIME入力列への変換
  - IPADIC、CC-CEDICT、拼音フォールバック辞書の内蔵
  - 自動解析中の手動入力タグと許可されたテキスト制御文字の保持
  - コメント、座標指定、スクリプト、クリア制御文字の解析対象からの除外

## リポジトリ構成

| パス | 内容 |
|---|---|
| `TypingEffect.obj2` | AviUtl2カスタムオブジェクト |
| `TypingEffect.mod2` | ビルド済みRustスクリプトモジュール |
| `README.txt` | 配布ユーザー向け使用方法 |
| `README.md` | ソース、ビルド、開発者向け説明 |
| `CHANGELOG.txt` | 変更履歴 |
| `aviutl2.toml` | aviutl2-cliプロジェクト設定 |
| `source/src/lib.rs` | Rustエンジン本体 |
| `source/Cargo.toml` | Rustパッケージ設定 |
| `source/build.ps1` | テスト、リリースビルド、ABI検査 |
| `source/check-syntax.ps1` | PowerShellとCargo設定の静的検査 |
| `source/verify-exports.ps1` | PE32+形式と必須エクスポートの検査 |
| `licenses/` | 依存ライセンス |

## Rust依存関係

| クレート | バージョン | 用途 |
|---|---:|---|
| `aviutl2` | 0.40.0 | AviUtl2スクリプトモジュールAPI |
| `lindera` | 4.0.0 | 日本語・中文の形態素解析 |
| `pinyin` | 0.11.0 | 中文の文字単位フォールバック |

有効なLindera機能は`embed-ipadic`と`embed-cc-cedict`です。`aviutl2`は公式サンプルと同様に既定機能を有効にしています。

```toml
aviutl2 = { version = "=0.40.0" }
lindera = { version = "=4.0.0", default-features = false, features = ["embed-ipadic", "embed-cc-cedict"] }
pinyin = { version = "=0.11.0", default-features = false, features = ["plain"] }
```

`aviutl2`の既定機能を無効にすると、モジュール内部で必要な`generic` / `filter`参照が解決できません。

## 必要な開発環境

- Windows 10またはWindows 11（64bit）
- Visual Studio 2022 Build Tools
  - C++によるデスクトップ開発
  - MSVC v143 x64/x86ビルドツール
  - Windows 10またはWindows 11 SDK
- Rust stable（MSVCツールチェーン）
- aviutl2-cli

Rustターゲットを準備します。

```powershell
rustup default stable-x86_64-pc-windows-msvc
rustup target add x86_64-pc-windows-msvc
```

aviutl2-cliをインストールします。

```powershell
cargo binstall aviutl2-cli
```

`cargo-binstall`を使用しない場合:

```powershell
cargo install aviutl2-cli
```

## aviutl2-cliで開発する

リポジトリルートで開発用AviUtl2を準備します。

```powershell
au2 prepare
```

ビルドしてAviUtl2を起動します。

```powershell
au2 dev
```

ログ追尾を行わない場合:

```powershell
au2 dev --detach
```

初回ビルドではIPADICとCC-CEDICTを取得・埋め込むため、通常より時間とディスク容量を使用します。

## CLIを使わずにビルドする

リポジトリルートで次を実行します。

```powershell
.\source\build.ps1
```

このスクリプトは次の処理を順番に実行します。

1. PowerShellスクリプトとCargo設定の検査
2. `x86_64-pc-windows-msvc`向けRustテスト
3. リリースビルド
4. `TypingEffect.mod2`のルートへのコピー
5. PE32+ / x64形式と必須エクスポートの検査

必須エクスポート:

- `GetScriptModuleTable`
- `InitializePlugin`
- `UninitializePlugin`

## 個別の検査

PowerShellとCargo設定の静的検査:

```powershell
.\source\check-syntax.ps1
```

Rustテスト:

```powershell
cd source
cargo test --target x86_64-pc-windows-msvc
```

オフラインキャッシュだけを使用する場合:

```powershell
cargo test --offline --target x86_64-pc-windows-msvc
```

ビルド済みモジュールのABI検査:

```powershell
.\source\verify-exports.ps1 -ModulePath .\TypingEffect.mod2
```

## 公開スクリプトモジュール関数

`TypingEffect.mod2`は次の関数を公開します。

| 関数 | 内容 |
|---|---|
| `annotate(文章, 言語)` | 文章をTypingEffect内部形式へ変換 |
| `available()` | 日本語・中文エンジンの利用可否を取得 |
| `last_error()` | 最後の解析エラーを取得 |
| `diagnose()` | 日本語・中文解析の診断結果を取得 |

言語番号は日本語が`0`、中文が`1`です。

## 配布パッケージ

リポジトリルートで次を実行します。

```powershell
au2 release
```

出力予定:

```text
release\TypingEffect_v0.9.11.au2pkg.zip
```

配布パッケージには`README.txt`を含め、開発者向け`README.md`は含めません。

## ライセンス

- TypingEffectソース: MIT
- aviutl2-rs: MIT
- Lindera: MIT
- pinyin: MIT
- IPADIC: `licenses/IPADIC_COPYING.txt`
- CC-CEDICT: CC BY-SA 4.0 (`licenses/CC-CEDICT_LICENSE.txt`)
