import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'

import { ErrorFallback } from '#components/error-fallback'

const meta = {
  title: 'Components/ErrorFallback',
  component: ErrorFallback,
  args: {
    resetErrorBoundary: fn(),
  },
} satisfies Meta<typeof ErrorFallback>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows the fallback after a data request fails.',
  args: {
    error: new Error('データの取得に失敗しました'),
  },
}

export const UnknownError: Story = {
  name: 'shows the fallback when the error value is unknown.',
  args: {
    error: 'unknown error',
  },
}
