import type { Meta, StoryObj } from '@storybook/react-vite'

import { ImportSbiDialog } from '@/components/trades/import-sbi-dialog'
import type { components } from '@/lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']

const strategies: Strategy[] = [
  {
    id: '00000000-0000-0000-0000-000000000001',
    name: '半導体短期スイング',
    description: null,
    sort_order: 0,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: '00000000-0000-0000-0000-000000000002',
    name: '高配当バリュー長期',
    description: null,
    sort_order: 1,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]

const meta = {
  title: 'Trades/ImportSbiDialog',
  component: ImportSbiDialog,
  parameters: { layout: 'centered' },
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
