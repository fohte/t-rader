import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { DeleteStrategyDialog } from '#components/strategy-settings/delete-strategy-dialog'
import { fetchClient } from '#lib/api/client'
import { createStoryRouter } from '#storybook/story-router'

// Storybook にはグローバルな QueryClientProvider が無いため、$api.useMutation を使う
// DeleteStrategyDialog 用にこのファイルの story 全体でモックを用意する
function installMiddleware() {
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url, method } = request
      if (method === 'DELETE' && /\/api\/strategies\/[^/]+$/.test(url)) {
        return new Response(null, { status: 204 })
      }
      if (method === 'GET' && /\/api\/strategies(\?|$)/.test(url)) {
        return new Response(JSON.stringify([]), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      return new Response(`unmocked request: ${method} ${url}`, {
        status: 404,
      })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

function QueryDecorator({
  defaultConfirmText,
}: {
  defaultConfirmText?: string
}) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  const [eject] = useState(() => installMiddleware())
  useEffect(() => eject, [eject])
  return (
    <QueryClientProvider client={client}>
      <DeleteStrategyDialog
        strategyId="strat-1"
        strategyName="半導体短期スイング"
        open
        onOpenChange={() => {}}
        defaultConfirmText={defaultConfirmText}
      />
    </QueryClientProvider>
  )
}

function createDialogRouter(defaultConfirmText?: string) {
  return createStoryRouter(
    () => <QueryDecorator defaultConfirmText={defaultConfirmText} />,
    { paths: ['/strategies'] },
  )
}

const meta = {
  title: 'StrategySettings/DeleteStrategyDialog',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows the confirmation dialog for deleting a strategy.',
  render: () => <RouterProvider router={createDialogRouter()} />,
}

export const NameMismatch: Story = {
  name: 'shows the delete button disabled when the confirmation name does not match.',
  render: () => <RouterProvider router={createDialogRouter('違う名前')} />,
}
