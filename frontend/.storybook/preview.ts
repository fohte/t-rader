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
      // レイアウト崩れではない
      globalIgnoreSelectors: [
        '.react-flow__node-graphNode',
        '[data-slot="graph-node"]',
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
