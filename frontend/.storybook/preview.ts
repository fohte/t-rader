import '../src/index.css'

import { configureOverflowCheck } from '@fohte/storybook-addon/preview'
import { withThemeByClassName } from '@storybook/addon-themes'
import type { Preview, Renderer } from 'storybook'

// react-flow の Handle (接続点、opacity: 0 で不可視) は react-flow 自身の既定 CSS
// で辺の境界線上に半分はみ出す形で描画され、CiteBadge はカード右上の角に半分
// はみ出す形で意図的に配置される。どちらも clip する祖先が無いため常に全体が
// 見えており、overflow-check が拾う「はみ出し」ではあるが実害のあるレイアウト
// 崩れではない
configureOverflowCheck({
  ignoreSelectors: ['.react-flow__node-graphNode', '[data-slot="graph-node"]'],
})

const preview: Preview = {
  decorators: [
    withThemeByClassName<Renderer>({
      themes: {
        light: '',
        dark: 'dark',
      },
      defaultTheme: 'light',
    }),
  ],
}

export default preview
