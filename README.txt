TypingEffect v0.5.3 — Full Rust / 日本語・中文対応
=================================================

AviUtl2向けIMEタイピングアニメーションです。
TypingEffect.mod2はRustだけで構築し、AviUtl2との接続にはaviutl2クレートを使用します。

日本語はLindera + IPADIC、中文はLindera + CC-CEDICTで自動解析します。
両辞書と拼音フォールバック辞書はTypingEffect.mod2へ内蔵されます。
外部DLL・外部辞書・設定ファイルは不要です。

このZIPはソース版です。ビルド済みTypingEffect.mod2は含みません。

■ 収録ファイル

  TypingEffect.obj2      タイピング本体
  checker.obj2           日本語・中文の動作確認
  aviutl2.toml           aviutl2-cli設定
  source\                Rustソースとビルドスクリプト
  licenses\              依存ライセンス

=================================================
対応機能
=================================================

■ 日本語

・Lindera + IPADICによる自動解析
・カタカナ読みからローマ字入力列を生成
・文節単位で入力、変換、確定
・手動入力指定: <i>kyou</i>今日

■ 中文

・Lindera + CC-CEDICTによる単語分割
・簡体字と繁体字の両方を解析
・辞書の拼音を声調なしのIME入力列へ変換
・声調数字1～5を除去
・ü / u: をvへ変換
・西安のようにa/e/o音節が続く場合は xi'an 形式を使用
・辞書に読みがない漢字はpinyinクレートで文字単位に補完
・拼音入力中は英字をそのまま未確定文字として表示
・注音は手動指定で対応

自動拼音の例:

  入力: 今天天气很好
  動作: jintian → 今天 / tianqi → 天气 / hen → 很 / hao → 好

繁体字の例:

  入力: 今天天氣很好

手動拼音:

  <i>jintian</i>今天

手動注音:

  <i>ㄐㄧㄣ ㄊㄧㄢ</i>今天

手動指定は自動解析より優先されます。
多音字・固有名詞など自動読みが意図と異なる箇所だけ手動指定できます。

■ 共通

・/ による変換区切り
・\/ によるスラッシュ表示
・入力中の下線
・入力中はカーソルを常時表示
・入力完了後の点滅カーソル
・左揃え、中央揃え、右揃え

=================================================
ビルドと確認
=================================================

【1】必要な環境を用意する

Visual Studio 2022 Build Toolsで次を有効にします。

  ・C++によるデスクトップ開発
  ・MSVC v143 x64/x86ビルドツール
  ・Windows 10またはWindows 11 SDK

RustをMSVC向けに設定します。

  rustup default stable-x86_64-pc-windows-msvc
  rustup target add x86_64-pc-windows-msvc

【2】aviutl2-cliを入れる

  cargo binstall aviutl2-cli

cargo-binstallを使わない場合:

  cargo install aviutl2-cli

確認:

  au2 --version

【3】開発用AviUtl2を準備する

aviutl2.tomlがあるフォルダーで実行します。

  au2 prepare

【4】ビルドして起動する

  au2 dev

ログ追尾なし:

  au2 dev --detach

初回ビルドではIPADICとCC-CEDICTを取得・埋め込むため、以前より時間と容量を使います。

【5】checkerで確認する

AviUtl2でカスタムオブジェクト「checker」を追加します。
正常なら次の情報が表示されます。

  OK: 日本語・中文を解析できました
  TypingEffect: 0.5.3
  辞書: IPADIC / CC-CEDICT（mod2内蔵）
  中文フォールバック: pinyin 0.11.0

【6】中文を確認する

カスタムオブジェクト「TypingEffect」を追加し、次のように設定します。

  言語: 中文
  自動解析: ON
  文章: 今天天气很好

拼音が英字で入力されたあと、中文へ変換されれば成功です。
繁体字も同様に確認できます。

  今天天氣很好

【7】配布パッケージを作る

  au2 release

出力予定:

  release\TypingEffect_v0.5.3.au2pkg.zip

=================================================
CLIを使わずにビルドする場合
=================================================

通常のPowerShellで実行できます。Visual Studio Build ToolsはRust MSVCのリンクに必要ですが、dumpbinは不要です。

  .\source\build.ps1

生成物:

  TypingEffect.mod2

次の3ファイルをAviUtl2の同じスクリプトフォルダーへ配置します。

  TypingEffect.obj2
  TypingEffect.mod2
  checker.obj2

=================================================
実装情報
=================================================

■ Rust依存関係

  aviutl2 0.40.0  default features enabled (module / generic / filter など)
  lindera 4.0.0   features = ["embed-ipadic", "embed-cc-cedict"]
  pinyin 0.11.0   features = ["plain"]


■ aviutl2クレートの機能設定

aviutl2 0.40.0では、module機能だけを有効にして他の既定機能を無効化すると、
クレート内部のgeneric / filter参照が解決できずビルドに失敗します。
この版では公式サンプルと同様に既定機能を有効にしています。

  aviutl2 = { version = "=0.40.0" }

次の旧設定は使用しないでください。

  aviutl2 = { version = "=0.40.0", default-features = false, features = ["module"] }

■ 公開スクリプトモジュール関数

  annotate(文章, 言語)
  available()
  last_error()
  diagnose()

■ 自動解析の注意

CC-CEDICTは語彙辞書を利用した分割・読み取得です。
文脈による多音字判定が常に完全とは限りません。
意図と異なる場合は次の形式で読みを固定してください。

  <i>chongqing</i>重庆

注音入力は自動生成せず、手動指定のみです。

■ ライセンス

・TypingEffectソース: MIT
・aviutl2-rs: MIT
・Lindera: MIT
・pinyin: MIT
・IPADIC: licenses\IPADIC_COPYING.txt
・CC-CEDICT: CC BY-SA 4.0
  licenses\CC-CEDICT_LICENSE.txt

------------------------------------------------------------
構文だけを確認する
------------------------------------------------------------

通常のPowerShellでsourceフォルダーへ移動し、
次を実行してください。

  powershell -NoProfile -ExecutionPolicy Bypass -File .\check-syntax.ps1

この検査はbuild.ps1、verify-exports.ps1、check-syntax.ps1を
PowerShellのASTパーサーで解析し、Cargo.tomlをcargo metadataで解析します。


=================================================
v0.5.3でのビルド検査変更
=================================================

verify-exports.ps1はdumpbin.exeを使用しません。
PowerShellだけでPE32+ / x64 / エクスポートテーブルを解析し、
次の3関数を確認します。

  GetScriptModuleTable
  InitializePlugin
  UninitializePlugin

そのため、au2 devは通常のPowerShellから実行できます。
