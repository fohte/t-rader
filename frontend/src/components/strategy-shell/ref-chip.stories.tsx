import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { RefChip } from '#components/strategy-shell/ref-chip'
import { mockResolveRef } from '#storybook/mock-resolve-ref'

const queryClient = new QueryClient()

const NAMES: Record<string, string> = {
  'stock:7203': 'トヨタ自動車',
  'stock:3436': 'SUMCO',
  'indicator:USDJPY': 'USD/JPY',
  'sector:半導体': '半導体',
  'theme:円安': '円安',
}

const meta = {
  title: 'StrategyShell/RefChip',
  component: RefChip,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  parameters: {
    msw: { handlers: [mockResolveRef(NAMES)] },
  },
} satisfies Meta<typeof RefChip>

export default meta
type Story = StoryObj<typeof meta>

export const Stock: Story = {
  name: 'shows a stock reference with its resolved company name.',
  args: { token: 'stock:7203' },
}

export const Indicator: Story = {
  name: 'shows an indicator reference with its resolved label.',
  args: { token: 'indicator:USDJPY' },
}

export const Sector: Story = {
  name: 'shows a sector reference with its resolved name.',
  args: { token: 'sector:半導体' },
}

export const Theme: Story = {
  name: 'shows a theme reference with its resolved name.',
  args: { token: 'theme:円安' },
}

export const Pill: Story = {
  name: 'shows a stock reference in the compact pill style.',
  args: { token: 'stock:3436', pill: true },
}

export const Unknown: Story = {
  name: 'shows an unresolved stock reference.',
  args: { token: 'stock:9999' },
}
