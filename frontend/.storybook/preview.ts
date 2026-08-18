import '#index.css'

import { withThemeByClassName } from '@storybook/addon-themes'
import type { Preview } from '@storybook/react-vite'

const preview: Preview = {
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
}

export default preview
