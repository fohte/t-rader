import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { NoteHeader } from '#components/note-detail/note-header'
import type { components } from '#lib/api/schema.gen'
import { mockResolveRef } from '#storybook/mock-resolve-ref'

type Note = components['schemas']['Note']

const queryClient = new QueryClient()

const NAMES: Record<string, string> = {
  'stock:3436': 'SUMCO',
  'indicator:USDJPY': 'USD/JPY',
  'sector:半導体': '半導体',
}

const note: Note = {
  id: '00000000-0000-0000-0000-000000000001',
  version_id: '00000000-0000-0000-0000-000000000101',
  version_no: 1,
  is_current: true,
  strategy_id: 'semi-swing',
  title: 'SUMCO レンジ回帰の確度評価',
  body_md: '[[stock:3436]] [[indicator:USDJPY]] [[sector:半導体]]',
  frontmatter_json: {},
  graphs_json: [],
  type_tag: 'thesis',
  status: 'unread',
  trigger: 'cron',
  trigger_label: '毎日 07:00 JST',
  created_by_kind: 'llm',
  created_at: '2026-05-29T07:02:00Z',
  updated_at: '2026-06-07T00:00:00Z',
}

const meta = {
  title: 'NoteDetail/NoteHeader',
  component: NoteHeader,
  parameters: {
    layout: 'padded',
    msw: { handlers: [mockResolveRef(NAMES)] },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="max-w-3xl bg-background p-5 text-foreground">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
} satisfies Meta<typeof NoteHeader>

export default meta
type Story = StoryObj<typeof meta>

export const LLMUnread: Story = {
  args: { note, strategyId: 'semi-swing' },
}

export const HumanApproved: Story = {
  args: {
    note: { ...note, created_by_kind: 'human', status: 'approved' },
    strategyId: 'semi-swing',
  },
}

export const NoStrategy: Story = {
  args: { note: { ...note, strategy_id: null }, strategyId: null },
}
