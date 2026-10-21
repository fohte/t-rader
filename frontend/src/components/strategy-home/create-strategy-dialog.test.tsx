import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { createStoryRouter } from '#storybook/story-router'

type CreateStrategyDialogComponent =
  typeof import('#components/strategy-home/create-strategy-dialog').CreateStrategyDialog

function readStrategyCreationOutput(
  requests: Array<{ path: string; method: string; body: unknown }>,
  pathname: string,
  dialogOpen: boolean,
) {
  return { requests, pathname, dialogOpen }
}

function renderOpenDialog(CreateStrategyDialog: CreateStrategyDialogComponent) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function DialogHarness() {
    const [open, setOpen] = useState(true)
    return <CreateStrategyDialog open={open} onOpenChange={setOpen} />
  }
  const router = createStoryRouter(
    () => (
      <QueryClientProvider client={queryClient}>
        <DialogHarness />
      </QueryClientProvider>
    ),
    {
      paths: ['/strategies/$id'],
      initialPath: '/strategies/demo-strategy',
    },
  )
  render(<RouterProvider router={router} />)
  return router
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.resetModules()
})

describe('CreateStrategyDialog', () => {
  it('名前の入力欄に汎用的な入力案内を表示する', async () => {
    const { CreateStrategyDialog } =
      await import('#components/strategy-home/create-strategy-dialog')
    renderOpenDialog(CreateStrategyDialog)
    const nameInput = await screen.findByLabelText('戦略名 *')

    expect(nameInput.getAttribute('placeholder')).toEqual(
      '名前を入力してください',
    )
  })

  it('説明を送信し、作成した戦略へ移動する', async () => {
    const requests: Array<{
      path: string
      method: string
      body: unknown
    }> = []
    vi.stubGlobal(
      'fetch',
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const request =
          input instanceof Request ? input : new Request(input, init)
        const path = new URL(request.url).pathname
        const method = request.method.toUpperCase()
        const body = method === 'POST' ? await request.clone().json() : null
        requests.push({ path, method, body })

        if (path === '/api/strategies' && method === 'POST') {
          return Response.json(
            {
              id: 'demo-strategy',
              name: '架空戦略',
              description: '架空の説明',
              sort_order: 0,
              created_at: '2099-01-01T00:00:00Z',
              updated_at: '2099-01-01T00:00:00Z',
            },
            { status: 201 },
          )
        }
        return Response.json([])
      },
    )

    const { CreateStrategyDialog } =
      await import('#components/strategy-home/create-strategy-dialog')
    const router = renderOpenDialog(CreateStrategyDialog)
    const user = userEvent.setup()
    await screen.findByLabelText('戦略名 *')
    await user.type(screen.getByLabelText('戦略名 *'), '  架空戦略  ')
    await user.type(screen.getByLabelText('説明'), ' 架空の説明 ')
    await user.click(screen.getByRole('button', { name: '作成' }))

    await waitFor(() => {
      expect(
        readStrategyCreationOutput(
          requests,
          router.state.location.pathname,
          screen.queryByRole('dialog') != null,
        ),
      ).toEqual({
        requests: [
          {
            path: '/api/strategies',
            method: 'POST',
            body: { name: '架空戦略', description: '架空の説明' },
          },
        ],
        pathname: '/strategies/demo-strategy',
        dialogOpen: false,
      })
    })
  })
})
