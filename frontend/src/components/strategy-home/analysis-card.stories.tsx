import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { AnalysisCard } from '#components/strategy-home/analysis-card'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']

const note: Note = {
  id: '00000000-0000-0000-0000-000000000001',
  strategy_id: 'semi-swing',
  title: 'SUMCO レンジ回帰の確度評価',
  body_md:
    '## 要約\nSUMCO [[stock:3436]] は約 2 ヶ月にわたり 1,480-1,640 のレンジで推移。[[indicator:USDJPY]] と [[sector:半導体]] のモメンタムは中立。テクニカルなレンジ回帰が機能しやすい局面と判断する。',
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

function createAnalysisCardRouter(props: { note: Note }) {
  return createStoryRouter(
    () => (
      <div className="max-w-[640px] bg-[color:var(--color-bg-primary)] p-4">
        <AnalysisCard note={props.note} strategyId="semi-swing" />
      </div>
    ),
    {
      paths: ['/strategies/$id', '/strategies/$id/notes/$noteId'],
      initialPath: '/strategies/semi-swing',
    },
  )
}

const meta = {
  title: 'StrategyHome/AnalysisCard',
  parameters: { layout: 'fullscreen' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => <RouterProvider router={createAnalysisCardRouter({ note })} />,
}

export const Approved: Story = {
  render: () => (
    <RouterProvider
      router={createAnalysisCardRouter({
        note: { ...note, status: 'approved' },
      })}
    />
  ),
}
