import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  HypothesisList,
  type HypothesisListItem,
} from '#components/hypothesis-list/hypothesis-list'

afterEach(cleanup)

function renderInRouter(hypotheses: HypothesisListItem[]) {
  const rootRoute = createRootRoute({
    component: () => <HypothesisList hypotheses={hypotheses} />,
  })
  const detailRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/hypotheses/$hypothesisId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([detailRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })

  return render(<RouterProvider router={router} />)
}

describe('HypothesisList', () => {
  it('仮説 ID を詳細画面へのリンクに使う', async () => {
    const { container } = renderInRouter([
      {
        hypothesisId: 'hypothesis-example-1',
        title: '検証用の仮説',
        status: 'unverified',
        updatedAt: '3 分前',
      },
    ])

    await waitFor(() => {
      expect(
        Array.from(container.querySelectorAll('a')).map((link) => ({
          href: link.getAttribute('href'),
          text: link.textContent,
        })),
      ).toEqual([
        {
          href: '/hypotheses/hypothesis-example-1',
          text: '検証用の仮説未検証3 分前',
        },
      ])
    })
  })

  it('一覧が空なら空状態を表示する', async () => {
    const { container } = renderInRouter([])

    await waitFor(() => {
      expect(container.textContent).toBe('一覧0—')
    })
  })
})
