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
