import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { ImportSbiDialog } from '#components/trades/import-sbi-dialog'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']

const queryClient = new QueryClient()

function strategyStub(id: string, name: string, sortOrder: number): Strategy {
  return {
    id,
    name,
    description: null,
    sort_order: sortOrder,
    agents_md: '',
    skills: {},
    agent_graph: '',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  }
}

const strategies: Strategy[] = [
  strategyStub('00000000-0000-0000-0000-000000000001', '半導体短期スイング', 0),
  strategyStub('00000000-0000-0000-0000-000000000002', '高配当バリュー長期', 1),
]

const meta = {
  title: 'Trades/ImportSbiDialog',
  component: ImportSbiDialog,
  parameters: { layout: 'centered' },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
} satisfies Meta<typeof ImportSbiDialog>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    open: true,
    onOpenChange: () => {},
    strategies,
  },
}
