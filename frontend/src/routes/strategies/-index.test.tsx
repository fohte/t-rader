import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.resetModules()
})

function getStrategyTabOutput(search: unknown) {
  return {
    search,
    tabs: screen.getAllByRole('tab').map((tab) => ({
      label: tab.textContent,
      selected: tab.getAttribute('aria-selected') === 'true',
    })),
    heading: screen.getByRole('heading', { level: 1 }).textContent,
  }
}

describe('戦略一覧のタブ', () => {
  it.each([
    { initialTab: 'list', nextTab: 'runs', expectedHeading: '実行履歴' },
    { initialTab: 'runs', nextTab: 'list', expectedHeading: '> 戦略を選ぶ' },
  ] as const)(
    '$initialTab から $nextTab を選ぶと URL と表示が更新される',
    async ({ initialTab, nextTab, expectedHeading }) => {
      vi.stubGlobal('fetch', () => Promise.resolve(Response.json([])))
      const { routeTree } = await import('#routeTree.gen')
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      })
      const router = createRouter({
        routeTree,
        history: createMemoryHistory({
          initialEntries: [`/strategies/?strategyTab=${initialTab}`],
        }),
      })
      render(
        <QueryClientProvider client={queryClient}>
          <RouterProvider router={router} />
        </QueryClientProvider>,
      )

      const user = userEvent.setup()
      await screen.findByRole('tablist', { name: '戦略一覧のタブ' })
      await user.click(
        screen.getByRole('tab', {
          name: nextTab === 'list' ? '一覧' : '実行履歴',
        }),
      )

      await waitFor(() => {
        expect(getStrategyTabOutput(router.state.location.search)).toEqual({
          search: { strategyTab: nextTab },
          tabs: [
            { label: '一覧', selected: nextTab === 'list' },
            { label: '実行履歴', selected: nextTab === 'runs' },
          ],
          heading: expectedHeading,
        })
      })
    },
    15_000,
  )
})
