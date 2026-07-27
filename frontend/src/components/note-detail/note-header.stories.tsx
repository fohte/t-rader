import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteHeader } from '#components/note-detail/note-header'
import type { components } from '#lib/api/schema.gen'

type Note = components['schemas']['Note']

const note: Note = {
  id: '00000000-0000-0000-0000-000000000001',
  strategy_id: 'semi-swing',
  title: 'SUMCO レンジ回帰の確度評価',
  body_md: '[[stock:3436]] [[indicator:USDJPY]] [[sector:半導体]]',
  frontmatter_json: {},
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
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="max-w-[720px] bg-[color:var(--color-bg-primary)] p-5 text-[color:var(--color-text-primary)]">
        <Story />
      </div>
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
