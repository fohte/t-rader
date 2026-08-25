# Design system

t-rader フロントエンドのデザインシステムのトークン契約。
ニアモノクロダーク + 赤アクセント 1 色 + monospace UI chrome という構成は、同じ作者が管理する [tq](https://github.com/fohte/tq) と土台を共有しており、トークン名と値もできる限り tq に揃えている。
tq と構造を揃えておくことで、両リポジトリ間の差分を把握して吸収するコストを下げる狙いがある。

このドキュメントの値の出典は `frontend/src/index.css` の `:root` / `@theme inline` ブロック。
ドキュメントと実装が食い違った場合は実装が正で、同じ PR でこのドキュメントを直す。

## Design tokens

すべてのトークンは `:root` 直下にのみ定義する。
light 用パレットは存在しない (`frontend/index.html` が `class="dark"` 固定でテーマ切り替えもないため)。

### Surfaces

| Token                  | 値        | Tailwind utility    | 用途                                        |
| ---------------------- | --------- | ------------------- | ------------------------------------------- |
| `--background`         | `#0a0a0a` | `bg-background`     | ページ背景                                  |
| `--card`               | `#141414` | `bg-card`           | カードやポップオーバーなど一段上げた面      |
| `--popover`            | `#141414` | `bg-popover`        | ポップオーバー/メニュー面 (`--card` と同値) |
| `--secondary`          | `#141414` | `bg-secondary`      | secondary fill                              |
| `--muted`              | `#141414` | `bg-muted`          | muted fill (hover 背景など)                 |
| `--accent`             | `#141414` | `bg-accent`         | accent fill (メニュー item hover など)      |
| `--surface-strong`     | `#1f1f1f` | `bg-surface-strong` | 強調した面 (active tab、primary button 等)  |
| `--color-bg-secondary` | `#0f0f0f` | `bg-bg-secondary`   | t-rader 固有。background と card の中間段   |
| `--color-bg-tertiary`  | `#1a1a1a` | `bg-bg-tertiary`    | t-rader 固有。card よりさらに上げた面       |

### Text

| Token                       | 値        | Tailwind utility               | 用途                                                   |
| --------------------------- | --------- | ------------------------------ | ------------------------------------------------------ |
| `--foreground`              | `#fafafa` | `text-foreground`              | 主要テキスト                                           |
| `--muted-foreground-strong` | `#a1a1aa` | `text-muted-foreground-strong` | foreground と muted-foreground の中間の secondary text |
| `--muted-foreground`        | `#71717a` | `text-muted-foreground`        | 標準の secondary/muted text                            |
| `--card-foreground`         | `#fafafa` | `text-card-foreground`         | `--card` 面上のテキスト                                |
| `--popover-foreground`      | `#fafafa` | `text-popover-foreground`      | `--popover` 面上のテキスト                             |
| `--secondary-foreground`    | `#fafafa` | `text-secondary-foreground`    | `--secondary` 面上のテキスト                           |
| `--accent-foreground`       | `#fafafa` | `text-accent-foreground`       | `--accent` 面上のテキスト                              |

グレー階調は明るい順に `--foreground`、`--muted-foreground-strong`、`--muted-foreground` と並ぶ。
新しいグレー値を作らず、既存のいずれかの階調を使うこと。

### Borders

| Token      | 値        | Tailwind utility | 用途                                         |
| ---------- | --------- | ---------------- | -------------------------------------------- |
| `--border` | `#2a2a2a` | `border-border`  | 標準の 1px border (デフォルト)               |
| `--input`  | `#2a2a2a` | `border-input`   | フォーム input の border (`--border` と同値) |

### Accent (唯一の色)

| Token                  | 値        | Tailwind utility                                 | 用途                                                                      |
| ---------------------- | --------- | ------------------------------------------------ | ------------------------------------------------------------------------- |
| `--primary`            | `#ef4444` | `text-primary` / `bg-primary` / `border-primary` | 唯一の赤アクセント。強調テキスト、アイコン、focus など punctuation に使う |
| `--primary-foreground` | `#fafafa` | `text-primary-foreground`                        | `bg-primary` 上のテキスト                                                 |
| `--destructive`        | `#ef4444` | `text-destructive` / `border-destructive`        | `--primary` と同じ赤 (accent と danger を同じ 1 色で表現する)             |
| `--ring`               | `#ef4444` | `ring-ring`                                      | focus ring 色                                                             |

### Sidebar

| Token                          | 値        | Tailwind utility                              |
| ------------------------------ | --------- | --------------------------------------------- |
| `--sidebar`                    | `#0a0a0a` | `bg-sidebar`                                  |
| `--sidebar-foreground`         | `#fafafa` | `text-sidebar-foreground`                     |
| `--sidebar-primary`            | `#ef4444` | `bg-sidebar-primary` / `text-sidebar-primary` |
| `--sidebar-primary-foreground` | `#fafafa` | `text-sidebar-primary-foreground`             |
| `--sidebar-accent`             | `#141414` | `bg-sidebar-accent`                           |
| `--sidebar-accent-foreground`  | `#fafafa` | `text-sidebar-accent-foreground`              |
| `--sidebar-border`             | `#2a2a2a` | `border-sidebar-border`                       |
| `--sidebar-ring`               | `#ef4444` | `ring-sidebar-ring`                           |

### 株価の方向 (t-rader 固有)

tq に対応物はない。
日本の慣習に合わせて上げを赤、下げを青にしている。

| Token              | 値        | Tailwind utility        | 用途               |
| ------------------ | --------- | ----------------------- | ------------------ |
| `--color-up`       | `#ef5350` | `text-up` / `bg-up`     | 上げ               |
| `--color-up-dim`   | `#5a2a2a` | (var 参照のみ)          | 上げの控えめな塗り |
| `--color-down`     | `#3f9fe0` | `text-down` / `bg-down` | 下げ               |
| `--color-down-dim` | `#234152` | (var 参照のみ)          | 下げの控えめな塗り |

### レビューとタスク実行のステータス (t-rader 固有)

tq に対応物はない。

| Token                   | 値        | Tailwind utility         | 用途                                           |
| ----------------------- | --------- | ------------------------ | ---------------------------------------------- |
| `--status-approved`     | `#4ea1a1` | `bg-status-approved`     | アーティファクトの承認済み                     |
| `--status-unread`       | `#ef4444` | `bg-status-unread`       | 未読                                           |
| `--status-rejected`     | `#71717a` | `bg-status-rejected`     | 却下                                           |
| `--status-task-running` | `#fbbf24` | `bg-status-task-running` | 戦略タスク実行中 (failed の赤と衝突しない別色) |

### Radius

`--radius` は `0.625rem` (tq は `0rem`)。
tq と揃えるかどうかは全画面の見た目に関わる別種の変更になるため、このトークン整理 PR のスコープ外としている。

## 互換エイリアス

strategy tokens 導入時に定義した旧トークン名は、上記の新トークンを指すエイリアスとして残っている。
役割は同じで名前だけが違う。
アーティファクト種別ごとの arbitrary value 置換 PR が並列に進む間、`index.css` を一切変更せずに済むようにするための互換レイヤーで、それらの PR がすべて merge されたら削除し、`eslint.config.js` の `tailwindcss/no-arbitrary-value: 'off'` も外す。

| 旧トークン名              | 新トークン (エイリアス先)   |
| ------------------------- | --------------------------- |
| `--color-bg-primary`      | `--background`              |
| `--color-text-primary`    | `--foreground`              |
| `--color-text-secondary`  | `--muted-foreground-strong` |
| `--color-text-tertiary`   | `--muted-foreground`        |
| `--color-border-strategy` | `--border`                  |
| `--color-accent-strategy` | `--primary`                 |
| `--panel`                 | `--card`                    |
| `--panel-inset`           | `--surface-strong`          |
| `--hairline`              | `--border`                  |

新規コードは旧トークン名ではなく上記表の新トークン名 (または対応する Tailwind utility) を使うこと。

## Fonts

| Role        | CSS 変数           | フォントスタック                                                                                    | Tailwind utility                | 用途                                   |
| ----------- | ------------------ | --------------------------------------------------------------------------------------------------- | ------------------------------- | -------------------------------------- |
| Sans        | `--font-sans`      | Inter, Helvetica Neue, Arial, Hiragino Kaku Gothic ProN, Noto Sans JP, sans-serif                   | `font-sans` (html に適用、既定) | 本文                                   |
| Mono (UI)   | `--font-mono-ui`   | JetBrains Mono Variable, IBM Plex Mono, SFMono-Regular, Consolas, Liberation Mono, Menlo, monospace | `font-mono` / `font-mono-ui`    | UI chrome (ラベル、数値、コード的表示) |
| Mono (body) | `--font-mono-body` | `--font-mono-ui` と同じ                                                                             | `font-mono-body`                | mono な本文                            |

`--font-mono` (Tailwind 既定の utility) は `--font-mono-ui` のエイリアスにしてある。
既存コードの `font-mono` 呼び出し (300 箇所超) を書き換えずに新フォントへ切り替えるため。

`JetBrains Mono Variable` / `IBM Plex Mono` は `@fontsource-variable/jetbrains-mono` / `@fontsource/ibm-plex-mono` (400/500/600) を `frontend/src/index.css` の `@import` で読み込んでいる。
フォールバックにのみ頼らないこと。

## Typography scale

Tailwind 標準の `text-*` スケールに加え、それより小さい段が 1 つだけある。

| Token        | 値                                                 | Tailwind utility | 用途                               |
| ------------ | -------------------------------------------------- | ---------------- | ---------------------------------- |
| `--text-2xs` | `0.6875rem` (11px)、line-height `0.9375rem` (15px) | `text-2xs`       | 最小段の mono UI chrome (ラベル等) |

新しい `--text-*` の段を追加する前に、既存の `text-2xs` で表現できないか確認すること。

arbitrary value 置換 PR で `text-[Npx]` を見つけたら、以下の表で標準スケールへ丸めること。Spacing scale と同じく同点は切り上げ、±1px の視覚的なズレは許容する。

| 現状の arbitrary value | 丸め先            | 理由                                                        |
| ---------------------- | ----------------- | ----------------------------------------------------------- |
| `10px`                 | `text-2xs` (11px) | `text-2xs` に最も近い                                       |
| `13px`                 | `text-sm` (14px)  | `text-xs`(12px) と `text-sm`(14px) の中間、同点は切り上げ   |
| `17px`                 | `text-lg` (18px)  | `text-base`(16px) と `text-lg`(18px) の中間、同点は切り上げ |
| `19px`                 | `text-xl` (20px)  | `text-lg`(18px) と `text-xl`(20px) の中間、同点は切り上げ   |
| `22px`                 | `text-2xl` (24px) | `text-xl`(20px) と `text-2xl`(24px) の中間、同点は切り上げ  |

丸めても許容しないのは、隣接する要素との相対的なサイズ関係 (親ラベルより小さい従属バッジ等) が崩れることで、表を機械的に適用する前に確認すること。`9px` のように `text-2xs` (11px) より丸め先が離れすぎ、かつ隣接要素との意図的なサイズ差を保つために必要な値は、丸めずに arbitrary value のまま据え置く。

## Spacing scale

`--spacing` は上書きしておらず、Tailwind 既定のグリッド (0.25rem = 4px 刻み、`0.5`/`1.5`/`2.5`/`3.5` の半段を含む) をそのまま使う。

arbitrary value 置換 PR で `[Npx]` 系の値を見つけたら、以下の表で機械的にグリッド上の段へ丸めること。
対象は `gap-`、`p`/`px`/`py`/`pt`/`pr`/`pb`/`pl`、`m`/`mx`/`my`/`mt`/`mr`/`mb`/`ml`、`w`/`h`/`min-w`/`min-h`/`max-w`/`max-h` (ただし要素自体のサイズが 44px を超える場合はグリッド丸めの対象外で、個別に名前付きトークンを検討する)。

| 現状の arbitrary value | 丸め先       | 理由                                                          |
| ---------------------- | ------------ | ------------------------------------------------------------- |
| `3px`                  | `1` (4px)    | `0.5`(2px) と `1`(4px) の中間、同点は切り上げ                 |
| `5px`                  | `1.5` (6px)  | `1`(4px) と `1.5`(6px) の中間、同点は切り上げ                 |
| `7px`                  | `2` (8px)    | `1.5`(6px) と `2`(8px) の中間、同点は切り上げ                 |
| `9px`                  | `2.5` (10px) | `2`(8px) と `2.5`(10px) の中間、同点は切り上げ                |
| `11px`                 | `3` (12px)   | `2.5`(10px) と `3`(12px) の中間、同点は切り上げ               |
| `13px`                 | `3.5` (14px) | `3`(12px) と `3.5`(14px) の中間、同点は切り上げ               |
| `18px`                 | `5` (20px)   | `4`(16px) と `5`(20px) の中間、同点は切り上げ                 |
| `22px`                 | `6` (24px)   | `5`(20px) と `6`(24px) の中間、同点は切り上げ                 |
| `41px`                 | `10` (40px)  | 40px の方が近い (44px は tap target サイズとして意味が変わる) |
| `44px`                 | `11` (44px)  | すでにグリッド上 (`11 × 4px`)。bracket を外すだけ             |

±1px の視覚的なズレは許容する。
許容しないのは順序関係 (見出し vs 本文など) が崩れることで、表を機械的に適用する前に確認すること。
border-width (`border`、`border-<N>`) は `--spacing` 由来ではなく `<N>px` に直接解決するため、この表の対象外。
`grid-template-columns` の複合トラック定義 (例: `grid-cols-[64px_1fr]`、`grid-cols-[minmax(0,1fr)_360px]`) も、固定幅と可変幅を 1 つの utility で表現する対応先が存在しないため対象外。
親要素のフォントサイズに対する相対値 (例: `text-[0.78em]`) も、Tailwind の `--text-*` namespace が絶対値前提のため対象外。

丸め先の標準スケール段が隣接要素と同じサイズになり、意図的なサイズ差 (親ラベルより小さい従属バッジ等) が失われる場合も丸めない。
例: `text-[9px]` の stale バッジは `text-2xs` (11px) に丸めると親ラベルと同サイズになり階層が消えるため据え置く。

44px を超えて丸め対象外になった値のうち、viewport 単位を含むなど `--spacing` の倍数で表現できないものは、`:root` に素の名前 (例: `--floating-chat-max-w`) で token を定義したうえで、`@theme inline` 側に `--spacing-<name>` として re-export し、この節の表に追記すること。

| Token                   | 値                      | Tailwind utility        | 用途                                                                   |
| ----------------------- | ----------------------- | ----------------------- | ---------------------------------------------------------------------- |
| `--floating-chat-max-w` | `calc(100vw - 1.75rem)` | `max-w-floating-chat-w` | floating chat panel の最大幅 (viewport 幅から左右マージン分を引く)     |
| `--floating-chat-max-h` | `calc(100vh - 6.25rem)` | `max-h-floating-chat-h` | floating chat panel の最大高さ (viewport 高さから上下マージン分を引く) |

## Non-goals

このドキュメントはトークン契約であって、既存画面の一括 restyle ではない。
以下は意図的にスコープ外としている。

- 既存コンポーネントの arbitrary value をトークンに置き換える作業 (ディレクトリ単位の後続 PR で進める)
- `eslint.config.js` の `tailwindcss/no-arbitrary-value: 'off'` を外して lint で強制すること (後続 PR がすべて merge された後の最後の PR で行う)
- tq の primitives (`Panel` / `Chip` / `TabStrip` 等) の移植
- `--radius` を tq (`0rem`) に揃えること
