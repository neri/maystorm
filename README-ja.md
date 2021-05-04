# MEG-OS codename Maystorm

Rust で再実装された MEG-OS

## 特徴

* 主要なコードが Rust で書かれた自作 OS
* UEFI で起動する 64bit OS
* 64 コアまでのマルチコアに対応
* WebAssembly のサポート

## Haribote-OS 互換サブシステム

* 現時点で約半数のアプリが動作することを確認しています。一部のAPIは未実装です。
* このサブシステムは、将来的にサポートされない可能性や、アーキテクチャが変更される可能性があります。
