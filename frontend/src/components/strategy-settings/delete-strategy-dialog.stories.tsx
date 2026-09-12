import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'
import { expect, userEvent, waitFor, within } from 'storybook/test'

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

function QueryDecorator() {
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
      />
    </QueryClientProvider>
  )
}

function createDialogRouter() {
  return createStoryRouter(() => <QueryDecorator />, {
    paths: ['/strategies'],
  })
}

const meta = {
  title: 'StrategySettings/DeleteStrategyDialog',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => <RouterProvider router={createDialogRouter()} />,
}

export const NameMismatch: Story = {
  render: () => <RouterProvider router={createDialogRouter()} />,
  play: async ({ canvasElement }) => {
    // DeleteStrategyDialog は Portal で document.body 直下に描画されるため、
    // canvasElement ではなく canvasElement.ownerDocument.body 側でスコープする
    const canvas = within(canvasElement.ownerDocument.body)
    const input = await canvas.findByLabelText(/確認のため戦略名/)
    await userEvent.type(input, '違う名前')
    await waitFor(() => expect(input).toHaveValue('違う名前'))
    const deleteButton = canvas.getByRole('button', { name: '削除する' })
    await expect(deleteButton).toBeDisabled()
  },
}
