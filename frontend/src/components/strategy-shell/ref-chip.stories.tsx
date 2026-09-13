import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { http, HttpResponse } from 'msw'

import { RefChip } from '#components/strategy-shell/ref-chip'

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
    msw: {
      handlers: [
        http.get('/api/refs/resolve', ({ request }) => {
          const link = new URL(request.url).searchParams.get('link') ?? ''
          const i = link.indexOf(':')
          return HttpResponse.json([
            {
              kind: i < 0 ? link : link.slice(0, i),
              id: i < 0 ? link : link.slice(i + 1),
              name: NAMES[link] ?? null,
            },
          ])
        }),
      ],
    },
  },
} satisfies Meta<typeof RefChip>

export default meta
type Story = StoryObj<typeof meta>

export const Stock: Story = {
  args: { token: 'stock:7203' },
}

export const Indicator: Story = {
  args: { token: 'indicator:USDJPY' },
}

export const Sector: Story = {
  args: { token: 'sector:半導体' },
}

export const Theme: Story = {
  args: { token: 'theme:円安' },
}

export const Pill: Story = {
  args: { token: 'stock:3436', pill: true },
}

export const Unknown: Story = {
  args: { token: 'stock:9999' },
}
