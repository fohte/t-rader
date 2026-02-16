import type { Preview, Renderer } from 'storybook'
import { withThemeByClassName } from '@storybook/addon-themes'

import '../src/index.css'

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
