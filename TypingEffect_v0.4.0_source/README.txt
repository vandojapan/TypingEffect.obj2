TypingEffect v0.4.0 — Lindera移行試作
=====================================

AviUtl2向けIMEタイピングアニメーションです。
名称を「TypingEffect」へ変更し、形態素解析エンジンをMeCabからLinderaへ置き換えます。

■ 今回の範囲

・日本語: Lindera 4.0.0 + 埋め込みIPADICで自動解析
・中文: 言語プルダウンを追加。自動解析は次版で実装予定
・中文でも <i>pinyin</i>漢字 / <i>注音</i>漢字 の手動指定は利用可能
・MeCab DLL、mecabrc、外部辞書フォルダーは不要

■ 完成時の配置

  TypingEffect.obj2
  TypingEffect.mod2
  TypingEffectLindera.dll
  形態素解析エンジン診断.obj2

すべて同じフォルダーへ置きます。

■ ビルド

必要環境:
・Visual Studio 2022 C++ビルドツール（x64）
・Rust stable-x86_64-pc-windows-msvc

x64 Native Tools PowerShell for VS 2022で次を実行します。

  rustup target add x86_64-pc-windows-msvc
  .\source\build.ps1

build.ps1は先にRust製TypingEffectLindera.dllを作り、続いて
C++製TypingEffect.mod2をビルドします。

■ 言語

  日本語 = 自動解析対応
  中文   = 現時点では手動入力対応

例:
  <i>jintian</i>今天
  <i>ㄐㄧㄣ ㄊㄧㄢ</i>今天

■ ライセンス

Lindera本体はMITライセンスです。
埋め込みIPADICのライセンス表示も配布時に同梱してください。
AviUtl2 SDKヘッダーのライセンスは source\sdk\LICENSE.txt を参照してください。
