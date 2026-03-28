// eslint-disable-next-line no-restricted-imports -- Storybook preview は src 外にあるため相対パスが必要
import '../src/index.css'

import { withThemeByClassName } from '@storybook/addon-themes'
import type { Preview, Renderer } from 'storybook'

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
