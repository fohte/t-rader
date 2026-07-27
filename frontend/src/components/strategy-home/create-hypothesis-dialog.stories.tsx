import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

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
} satisfies Meta<typeof CreateHypothesisDialog>

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  args: {
    strategyId: '00000000-0000-0000-0000-000000000001',
    open: true,
    onOpenChange: () => {},
  },
}
