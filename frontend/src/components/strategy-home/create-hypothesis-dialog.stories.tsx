import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { http, HttpResponse } from 'msw'

import { CreateHypothesisDialog } from '#components/strategy-home/create-hypothesis-dialog'

const queryClient = new QueryClient()

const meta = {
  title: 'StrategyHome/CreateHypothesisDialog',
  component: CreateHypothesisDialog,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  parameters: {
    msw: {
      handlers: [http.get('/api/strategies', () => HttpResponse.json([]))],
    },
  },
} satisfies Meta<typeof CreateHypothesisDialog>

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  args: {
    open: true,
    onOpenChange: () => {},
  },
}
