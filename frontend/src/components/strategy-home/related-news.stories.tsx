import type { Meta, StoryObj } from '@storybook/react-vite'

import { RelatedNewsView } from '@/components/strategy-home/related-news'

const meta = {
  title: 'StrategyHome/RelatedNews',
  component: RelatedNewsView,
  parameters: { layout: 'centered' },
} satisfies Meta<typeof RelatedNewsView>

export default meta
type Story = StoryObj<typeof meta>

export const WithItems: Story = {
  args: {
    isPending: false,
    items: [
      {
        id: '00000000-0000-0000-0000-000000000001',
        source: 'Yahoo! Japan',
        url: 'https://example.com/news/1',
        title: 'トヨタ自動車、通期決算で過去最高益を達成',
        body_snippet: '為替効果と販売台数増が貢献。',
        published_at: '2026-06-26T06:00:00Z',
        matched_refs: [
          { ref_kind: 'stock', ref_id: '7203', matched_term: 'トヨタ自動車' },
        ],
      },
      {
        id: '00000000-0000-0000-0000-000000000002',
        source: 'Bloomberg JP',
        url: 'https://example.com/news/2',
        title: '半導体株が一斉高、AI 需要の追い風続く',
        body_snippet: null,
        published_at: '2026-06-26T05:30:00Z',
        matched_refs: [
          { ref_kind: 'theme', ref_id: 'semi', matched_term: '半導体' },
        ],
      },
    ],
  },
  decorators: [
    (Story) => (
      <div className="w-[320px]">
        <Story />
      </div>
    ),
  ],
}

export const Empty: Story = {
  args: { isPending: false, items: [] },
  decorators: [
    (Story) => (
      <div className="w-[320px]">
        <Story />
      </div>
    ),
  ],
}

export const Loading: Story = {
  args: { isPending: true, items: null },
  decorators: [
    (Story) => (
      <div className="w-[320px]">
        <Story />
      </div>
    ),
  ],
}
