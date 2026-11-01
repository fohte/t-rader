import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

type Annotation = components['schemas']['Annotation']

function createAnnotation(
  id: string,
  status: string,
  text: string,
  timestamp: string,
): Annotation {
  return {
    id,
    target_kind: 'stock',
    target_symbol: 'DEMO-A',
    timestamp,
    price: null,
    text,
    linked_note_id: null,
    status,
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: timestamp,
    updated_at: timestamp,
  }
}

const unreadFirst = createAnnotation(
  'fake-annotation-a',
  'unread',
  '架空の未読メモ A',
  '2026-10-12T01:00:00Z',
)
const unreadSecond = createAnnotation(
  'fake-annotation-b',
  'unread',
  '架空の未読メモ B',
  '2026-10-11T01:00:00Z',
)
const approvedAnnotation = createAnnotation(
  'fake-annotation-approved',
  'approved',
  '架空の確認済みメモ',
  '2026-10-10T01:00:00Z',
)
const rejectedAnnotation = createAnnotation(
  'fake-annotation-rejected',
  'rejected',
  '架空の却下済みメモ',
  '2026-10-09T01:00:00Z',
)
const allAnnotations = [
  unreadFirst,
  unreadSecond,
  approvedAnnotation,
  rejectedAnnotation,
]

afterEach(() => {
  cleanup()
  resetFetch()
})

async function renderRoute(initialPath: string) {
  const { routeTree } = await import('#routeTree.gen')
  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  })
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  render(<RouterProvider router={router} />, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  })
  return { router, queryClient }
}

function createRequest(input: RequestInfo | URL, init?: RequestInit) {
  return input instanceof Request ? input : new Request(input, init)
}

function getLocationOutput(pathname: string, search: unknown) {
  return { pathname, search }
}

function getAllAndPendingAnnotationsOutput(
  annotationQueries: string[],
  remainingAllStatusesVisible: boolean[],
  unreadCountLabel: string | null,
  search: unknown,
) {
  return {
    annotationQueries,
    remainingAllStatusesVisible,
    unreadCountLabel,
    search,
  }
}

function getAnnotationListStateOutput(
  error: HTMLElement | null,
  empty: string | null,
) {
  return { error, empty }
}

function getApprovalFailureOutput(
  approvalRequests: string[],
  selectedCountVisible: string | null,
  remainingAnnotationSelected: string | null,
  unreadCountLabel: string | null,
) {
  return {
    approvalRequests,
    selectedCountVisible,
    remainingAnnotationSelected,
    unreadCountLabel,
  }
}

function getAnnotationReturnOutput(
  pathname: string,
  search: unknown,
  listView: string | null,
) {
  return { pathname, search, listView }
}

function getDirectAnnotationNavigationOutput(
  search: unknown,
  nextHref: string | null,
) {
  return { search, nextHref }
}

function getAllViewAnnotationNavigationOutput(
  pathname: string,
  search: unknown,
  annotationQueryStatuses: (string | null)[],
) {
  return {
    pathname,
    search,
    allAnnotationsQuery: annotationQueryStatuses.some(
      (status) => status == null,
    ),
  }
}

describe('/notes annotation integration', () => {
  it.each([
    {
      name: 'redirects the old review route',
      initialPath: '/review',
      expectedPathname: '/notes',
      expectedSearch: { view: 'pending' },
    },
    {
      name: 'redirects the old pending note route',
      initialPath: '/note-versions/pending',
      expectedPathname: '/notes',
      expectedSearch: { view: 'pending' },
    },
    {
      name: 'preserves view and note filters from the old annotation route',
      initialPath: '/annotations?view=pending&kind=sample-kind&tag=sample-tag',
      expectedPathname: '/notes',
      expectedSearch: {
        tab: 'annotations',
        view: 'pending',
        kind: 'sample-kind',
        tag: 'sample-tag',
      },
    },
  ])('$name', async ({ initialPath, expectedPathname, expectedSearch }) => {
    stubFetch(() => Promise.resolve(Response.json([])))
    const { router } = await renderRoute(initialPath)

    await waitFor(() => {
      expect(
        getLocationOutput(
          router.state.location.pathname,
          router.state.location.search,
        ),
      ).toEqual({
        pathname: expectedPathname,
        search: expectedSearch,
      })
    })
  })

  it('switches between all and unread annotations while keeping note filters in the URL', async () => {
    const user = userEvent.setup()
    const annotationQueries: (string | null)[] = []
    stubFetch((input, init) => {
      const request = createRequest(input, init)
      const url = new URL(request.url)
      if (url.pathname === '/api/notes/overview') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/annotations') {
        const status = url.searchParams.get('status')
        annotationQueries.push(status)
        return Promise.resolve(
          Response.json(
            status === 'unread' ? [unreadFirst, unreadSecond] : allAnnotations,
          ),
        )
      }
      return Promise.resolve(Response.json([]))
    })

    const { router } = await renderRoute(
      '/notes?tab=annotations&view=all&kind=sample-kind&tag=sample-tag',
    )

    await screen.findByRole('link', { name: '架空の未読メモ A' })
    await screen.findByRole('link', { name: '架空の確認済みメモ' })
    await screen.findByRole('link', { name: '架空の却下済みメモ' })
    await screen.findByLabelText('未読アノテーション 2 件')
    await user.click(screen.getByRole('button', { name: '未読・レビュー待ち' }))

    await waitFor(() => {
      expect(router.state.location.search).toEqual({
        tab: 'annotations',
        view: 'pending',
        kind: 'sample-kind',
        tag: 'sample-tag',
      })
    })
    await waitFor(() => {
      expect(
        getAllAndPendingAnnotationsOutput(
          [...new Set(annotationQueries)]
            .map((status) => status ?? 'all')
            .sort(),
          [
            screen.queryByRole('link', { name: '架空の確認済みメモ' }) != null,
            screen.queryByRole('link', { name: '架空の却下済みメモ' }) != null,
          ],
          screen.getByLabelText('未読アノテーション 2 件').textContent,
          router.state.location.search,
        ),
      ).toEqual({
        annotationQueries: ['all', 'unread'],
        remainingAllStatusesVisible: [false, false],
        unreadCountLabel: '2',
        search: {
          tab: 'annotations',
          view: 'pending',
          kind: 'sample-kind',
          tag: 'sample-tag',
        },
      })
    })
  }, 30_000)

  it('shows retrieval errors separately from an empty unread list', async () => {
    const user = userEvent.setup()
    stubFetch((input, init) => {
      const request = createRequest(input, init)
      const url = new URL(request.url)
      if (url.pathname === '/api/notes/overview') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/annotations') {
        return url.searchParams.has('status')
          ? Promise.resolve(Response.json([]))
          : Promise.resolve(Response.json({}, { status: 500 }))
      }
      return Promise.resolve(Response.json([]))
    })

    await renderRoute('/notes?tab=annotations&view=all')
    await screen.findByText('アノテーション一覧を読み込めませんでした。')
    await user.click(screen.getByRole('button', { name: '未読・レビュー待ち' }))
    await screen.findByText('未読・レビュー待ちのアノテーションはありません。')

    expect(
      getAnnotationListStateOutput(
        screen.queryByText('アノテーション一覧を読み込めませんでした。'),
        screen.getByText('未読・レビュー待ちのアノテーションはありません。')
          .textContent,
      ),
    ).toEqual({
      error: null,
      empty: '未読・レビュー待ちのアノテーションはありません。',
    })
  }, 30_000)

  it('keeps the failed and remaining annotations selected and refreshes the unread count', async () => {
    const user = userEvent.setup()
    let unreadAnnotations = [unreadFirst, unreadSecond]
    const approvalRequests: string[] = []
    stubFetch((input, init) => {
      const request = createRequest(input, init)
      const url = new URL(request.url)
      if (url.pathname === '/api/notes/overview') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/annotations') {
        return Promise.resolve(Response.json(unreadAnnotations))
      }
      if (
        url.pathname.startsWith('/api/annotations/') &&
        request.method === 'POST'
      ) {
        const annotationId = url.pathname.split('/')[3]
        if (annotationId == null)
          return Promise.resolve(Response.json({}, { status: 404 }))
        approvalRequests.push(annotationId)
        if (annotationId === unreadFirst.id) {
          unreadAnnotations = [unreadSecond]
          return Promise.resolve(Response.json(unreadFirst))
        }
        return Promise.resolve(Response.json({}, { status: 500 }))
      }
      return Promise.resolve(Response.json([]))
    })

    const { router } = await renderRoute('/notes?tab=annotations&view=pending')
    expect(router.state.location.search).toEqual({
      tab: 'annotations',
      view: 'pending',
    })
    await screen.findByRole('link', { name: '架空の未読メモ A' })
    const selectAll =
      await screen.findByLabelText('表示中のアノテーションをすべて選択')
    await user.click(selectAll)
    const approveButtons = await screen.findAllByRole('button', {
      name: '選択した 2 件を承認',
    })
    await user.click(
      approveButtons.at(0) ??
        screen.getByRole('button', { name: '選択した 2 件を承認' }),
    )
    await screen.findByText(
      '一部の承認に失敗しました。失敗した項目と後続の選択を残しています。',
    )
    await screen.findByLabelText('未読アノテーション 1 件')

    expect(
      getApprovalFailureOutput(
        approvalRequests,
        screen.getByText('1 件を選択中').textContent,
        screen
          .getByLabelText('架空の未読メモ B を選択')
          .getAttribute('aria-checked'),
        screen.getByLabelText('未読アノテーション 1 件').textContent,
      ),
    ).toEqual({
      approvalRequests: [unreadFirst.id, unreadSecond.id],
      selectedCountVisible: '1 件を選択中',
      remainingAnnotationSelected: 'true',
      unreadCountLabel: '1',
    })
  }, 30_000)

  it('keeps annotation navigation and the return link inside the opened view', async () => {
    const user = userEvent.setup()
    stubFetch((input, init) => {
      const request = createRequest(input, init)
      const url = new URL(request.url)
      if (url.pathname === '/api/notes/overview') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/note-kinds') {
        return Promise.resolve(Response.json([]))
      }
      if (url.pathname === '/api/annotations') {
        return Promise.resolve(
          Response.json(
            url.searchParams.get('status') === 'unread'
              ? [unreadFirst, unreadSecond]
              : allAnnotations,
          ),
        )
      }
      if (url.pathname.startsWith('/api/annotations/')) {
        const annotationId = url.pathname.split('/')[3]
        return Promise.resolve(
          Response.json(
            allAnnotations.find((annotation) => annotation.id === annotationId),
          ),
        )
      }
      if (url.pathname === '/api/comments' || url.pathname === '/api/history') {
        return Promise.resolve(Response.json([]))
      }
      return Promise.resolve(Response.json([]))
    })

    const { router } = await renderRoute(
      '/notes?tab=annotations&view=pending&kind=sample-kind&tag=sample-tag',
    )
    await user.click(
      await screen.findByRole('link', { name: '架空の未読メモ A' }),
    )
    await screen.findByRole('link', { name: '次へ →' })
    await user.click(screen.getByRole('link', { name: '次へ →' }))
    await screen.findByText('架空の未読メモ B')
    await waitFor(() => {
      expect(
        getLocationOutput(
          router.state.location.pathname,
          router.state.location.search,
        ),
      ).toEqual({
        pathname: '/annotations/fake-annotation-b',
        search: {
          tab: 'annotations',
          view: 'pending',
          kind: 'sample-kind',
          tag: 'sample-tag',
        },
      })
    })
    await user.click(
      screen.getByRole('link', { name: '< アノテーション一覧に戻る' }),
    )
    await screen.findByRole('link', { name: '架空の未読メモ A' })

    expect(
      getAnnotationReturnOutput(
        router.state.location.pathname,
        router.state.location.search,
        screen
          .getByRole('button', { name: '未読・レビュー待ち' })
          .getAttribute('aria-pressed'),
      ),
    ).toEqual({
      pathname: '/notes',
      search: {
        tab: 'annotations',
        view: 'pending',
        kind: 'sample-kind',
        tag: 'sample-tag',
      },
      listView: 'true',
    })
  }, 30_000)

  it('uses the view in a directly opened annotation URL without history state', async () => {
    stubFetch((input, init) => {
      const request = createRequest(input, init)
      const url = new URL(request.url)
      if (url.pathname === '/api/annotations') {
        return Promise.resolve(Response.json([unreadFirst, unreadSecond]))
      }
      if (url.pathname === '/api/annotations/fake-annotation-a') {
        return Promise.resolve(Response.json(unreadFirst))
      }
      if (url.pathname === '/api/comments' || url.pathname === '/api/history') {
        return Promise.resolve(Response.json([]))
      }
      return Promise.resolve(Response.json([]))
    })

    const { router } = await renderRoute(
      '/annotations/fake-annotation-a?tab=annotations&view=pending&kind=sample-kind&tag=sample-tag',
    )
    await screen.findByRole('link', { name: '次へ →' })

    expect(
      getDirectAnnotationNavigationOutput(
        router.state.location.search,
        screen.getByRole('link', { name: '次へ →' }).getAttribute('href'),
      ),
    ).toEqual({
      search: {
        tab: 'annotations',
        view: 'pending',
        kind: 'sample-kind',
        tag: 'sample-tag',
      },
      nextHref:
        '/annotations/fake-annotation-b?tab=annotations&view=pending&kind=sample-kind&tag=sample-tag',
    })
  }, 30_000)

  it('navigates through all annotations from a directly opened all-view URL', async () => {
    const user = userEvent.setup()
    const allViewQueryStatuses: (string | null)[] = []
    stubFetch((input, init) => {
      const request = createRequest(input, init)
      const url = new URL(request.url)
      if (url.pathname === '/api/annotations') {
        allViewQueryStatuses.push(url.searchParams.get('status'))
        return Promise.resolve(Response.json(allAnnotations))
      }
      if (url.pathname.startsWith('/api/annotations/')) {
        const annotationId = url.pathname.split('/')[3]
        return Promise.resolve(
          Response.json(
            allAnnotations.find((annotation) => annotation.id === annotationId),
          ),
        )
      }
      if (url.pathname === '/api/comments' || url.pathname === '/api/history') {
        return Promise.resolve(Response.json([]))
      }
      return Promise.resolve(Response.json([]))
    })

    const { router } = await renderRoute(
      '/annotations/fake-annotation-approved?tab=annotations&view=all&kind=sample-kind&tag=sample-tag',
    )
    await user.click(await screen.findByRole('link', { name: '次へ →' }))
    await screen.findByText('架空の却下済みメモ')

    await waitFor(() => {
      expect(
        getAllViewAnnotationNavigationOutput(
          router.state.location.pathname,
          router.state.location.search,
          allViewQueryStatuses,
        ),
      ).toEqual({
        pathname: '/annotations/fake-annotation-rejected',
        search: {
          tab: 'annotations',
          view: 'all',
          kind: 'sample-kind',
          tag: 'sample-tag',
        },
        allAnnotationsQuery: true,
      })
    })
  }, 30_000)
})
