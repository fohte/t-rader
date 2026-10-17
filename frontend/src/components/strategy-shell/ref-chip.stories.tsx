import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { RefChip } from '#components/strategy-shell/ref-chip'
import { mockResolveRef } from '#storybook/mock-resolve-ref'

const queryClient = new QueryClient()

const NAMES: Record<string, string> = {
  'stock:demo-code': 'Sample Stock',
  'indicator:demo-indicator': 'Sample Indicator',
  'group:demo-axis/demo-group': 'Sample Group',
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
  name: 'shows a stock reference with its resolved name.',
  args: { token: 'stock:demo-code', onOpenRef: () => undefined },
}

export const Indicator: Story = {
  name: 'shows a clickable indicator reference with its resolved name.',
  args: { token: 'indicator:demo-indicator', onOpenRef: () => undefined },
}

export const Group: Story = {
  name: 'shows a clickable group reference with its resolved name.',
  args: {
    token: 'group:demo-axis/demo-group',
    onOpenRef: () => undefined,
  },
}

export const Pill: Story = {
  name: 'shows a stock reference in the compact pill style.',
  args: { token: 'stock:demo-code', pill: true },
}

export const Highlighted: Story = {
  name: 'highlights a matching part of a resolved reference.',
  args: { token: 'stock:demo-code', commentTextHighlight: 'Sample' },
}

export const Unknown: Story = {
  name: 'shows an unresolved stock reference.',
  args: { token: 'stock:missing-code', onOpenRef: () => undefined },
}
