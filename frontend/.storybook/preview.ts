import '#index.css'

import {
  afterEach,
  beforeEach,
  configureUnhandledApiRequestCheck,
  reportUnhandledApiRequest,
} from '@fohte/storybook-addon/preview'
import { withThemeByClassName } from '@storybook/addon-themes'
import type { Preview } from '@storybook/react-vite'
import { setupWorker } from 'msw/browser'
import { mswLoader } from 'msw-storybook-addon/csf3'

configureUnhandledApiRequestCheck({ pathPrefixes: ['/api/'] })

// @fohte/storybook-addon publishes only a `./preview` subpath export (no
// preset/manager entry), so listing it in main.ts's `addons` never wires its
// beforeEach/afterEach checks — they must be spread into this project's own
// preview annotations to actually run.
const preview: Preview = {
  beforeEach,
  afterEach,
  parameters: {
    overflowCheck: {
      // react-flow の Handle (接続点、opacity: 0 で不可視) は react-flow 自身の既定
      // CSS で辺の境界線上に半分はみ出す形で描画され、CiteBadge はカード右上の角に
      // 半分はみ出す形で意図的に配置される。どちらも clip する祖先が無いため常に
      // 全体が見えており、overflow-check が拾う「はみ出し」ではあるが実害のある
      // レイアウト崩れではない。
      //
      // .react-flow__viewport (react-flow__container 由来で常にコンテナ 100% 幅の
      // position: absolute) は fitView の pan/zoom (非恒等変換) を transform で
      // 適用するラッパー div。transform 後のボックスがコンテナ境界を超えて
      // CSSOM 上の scrollable overflow region に算入されるため、.react-flow /
      // .react-flow__renderer / .react-flow__pane (いずれもこのラッパーの祖先)
      // の scrollWidth が押し上げられる。実際に描画されるノード/エッジは
      // fitView の fit 対象そのものでコンテナ内に収まっており、ラッパー自体は
      // 不可視領域なので実害はない
      globalIgnoreSelectors: [
        '.react-flow__node-graphNode',
        '[data-slot="graph-node"]',
        '.react-flow',
        '.react-flow__renderer',
        '.react-flow__pane',
        // react-flow のエッジラベルはブラウザのテキスト計測がサブピクセル単位で
        // 揺れ、scrollWidth が clientWidth をわずかに (数 px 未満) 上回ることがある。
        // 表示上はクリップされず読めているため実害のあるレイアウト崩れではない
        '.react-flow__edge-text',
        // Monaco Editor は独自のスクロール実装を持つ。.monaco-scrollable-element
        // (overflow: hidden) の子である .lines-content は
        // position: absolute; width/height: 16777216px に固定される
        // (レイアウト確定後も常にこの値。Chrome の描画上限 16777216px を使った
        // 仮想スクロール実装のため。
        // https://stackoverflow.com/questions/38905916/content-in-google-chrome-larger-than-16777216-px-not-being-rendered)。
        // これが .monaco-scrollable-element 自身の scrollWidth に常に現れ、その子の
        // カスタムスクロールバー (.scrollbar) も同じ理由で scrollWidth が
        // clientWidth を超える。.overflow-guard はこれらをまとめて overflow: hidden
        // で clip する外側のラッパーで、レイアウト未確定時に一時的に overflow を
        // 報告することがある。いずれも Monaco の設計上の値・挙動であり、実害のある
        // レイアウト崩れではない
        '.monaco-editor .overflow-guard',
        '.monaco-editor .monaco-scrollable-element',
        '.monaco-scrollable-element > .scrollbar',
      ],
    },
  },
  decorators: [
    withThemeByClassName({
      themes: {
        light: '',
        dark: 'dark',
      },
      defaultTheme: 'light',
    }),
  ],
  loaders: [
    mswLoader(async () => {
      const worker = setupWorker()
      await worker.start({
        onUnhandledRequest: (request, print) => {
          if (reportUnhandledApiRequest(request.url)) {
            print.error()
          }
        },
      })
      return worker
    }),
  ],
}

export default preview
