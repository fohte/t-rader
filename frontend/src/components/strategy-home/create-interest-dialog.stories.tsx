import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { CreateInterestDialog } from '#components/strategy-home/create-interest-dialog'

const queryClient = new QueryClient()

const meta = {
  title: 'StrategyHome/CreateInterestDialog',
  component: CreateInterestDialog,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
} satisfies Meta<typeof CreateInterestDialog>

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  args: {
    strategyId: '00000000-0000-0000-0000-000000000001',
    open: true,
    onOpenChange: () => {},
  },
}
