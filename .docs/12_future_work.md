# 将来の作業

## 1. Partial clear

v1.0 には入れません。  
将来の検討順は次の通りです。

1. manual page break + carry-forward
2. 選択した group を次 page に残す
3. group 単位の clear
4. 任意の partial clear

## 2. ペン圧

本当の pressure support は future work です。  
v1.0 でのアーキテクチャ要件は、それを塞がないことです。

## 3. Auto taper / pseudo-pressure

将来の checkbox レベル機能です。

- Auto taper の on / off
- 後から advanced control

推奨する将来 signal は次の通りです。

- path progress
- speed
- 局所 curvature
- start / end への近さ

## 4. Proxy media

v1.0 には入れません。  
project model を差し替えずにあとから proxy 生成を足せるよう、media 層を設計します。

## 5. GPU export compositor

将来の最適化に限ります。  
v1.0 の正しさを犠牲にしてはいけません。

## 6. Codec-pack 補助ツール

optional で codec を使える FFmpeg runtime を、provenance / compliance を明確にしながら取得するための将来ツールです。

## 7. Effect scripting

v1.0 の対象外です。  
将来、安全な expression layer や scripting API を足す可能性はありますが、最初に hot path へ入れてはいけません。
