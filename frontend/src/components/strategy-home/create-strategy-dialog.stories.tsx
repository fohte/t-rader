import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { useState } from 'react'

import { CreateStrategyDialog } from '#components/strategy-home/create-strategy-dialog'
import { createStoryRouter } from '#storybook/story-router'

function QueryDecorator() {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  return (
    <QueryClientProvider client={client}>
      <CreateStrategyDialog open onOpenChange={() => {}} />
    </QueryClientProvider>
  )
}

function createDialogRouter() {
  return createStoryRouter(() => <QueryDecorator />, {
    paths: ['/strategies/$id'],
  })
}

const meta = {
  title: 'StrategyHome/CreateStrategyDialog',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  name: 'shows the open dialog for creating a strategy.',
  render: () => <RouterProvider router={createDialogRouter()} />,
}
