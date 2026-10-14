import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { FloatingChat } from '#components/strategy-shell/floating-chat'
import { closeFloatingChat } from '#components/strategy-shell/floating-chat-store'

interface ApiMockRequest {
  method: string
  path: string
  options: unknown
}

const apiMock = vi.hoisted(() => ({
  request: vi.fn<(request: ApiMockRequest) => Promise<unknown>>(),
}))

vi.mock('#lib/api/client', async () => {
  const { useMutation, useQuery } = await import('@tanstack/react-query')

  return {
    $api: {
      useMutation: (method: string, path: string) =>
        useMutation({
          mutationFn: (options: unknown) =>
            apiMock.request({ method, path, options }),
        }),
      useQuery: (
        method: string,
        path: string,
        options?: unknown,
        queryOptions?: object,
      ) =>
        useQuery({
          queryKey: options == null ? [method, path] : [method, path, options],
          queryFn: () => apiMock.request({ method, path, options }),
          ...queryOptions,
        }),
      queryOptions: (method: string, path: string) => ({
        queryKey: [method, path],
      }),
    },
  }
})

interface CapturedRequest {
  method: string
  path: string
  body: string | null
}

function readTaskRequests(requests: CapturedRequest[]) {
  return requests
    .filter(({ path }) => path.includes('/chat') || path.includes('/tasks/'))
    .map(({ method, path, body }) => ({ method, path, body }))
}

function readRequestPath(path: string, options: unknown): string {
  return path
    .replace('{id}', readPathParam(options, 'id'))
    .replace('{task_id}', readPathParam(options, 'task_id'))
}

function readPathParam(options: unknown, param: string): string {
  if (
    options == null ||
    typeof options !== 'object' ||
    !('params' in options)
  ) {
    return ''
  }
  const params = options.params
  if (params == null || typeof params !== 'object' || !('path' in params)) {
    return ''
  }
  const routeParams = params.path
  if (routeParams == null || typeof routeParams !== 'object') return ''
  if (param === 'id' && 'id' in routeParams) {
    return typeof routeParams.id === 'string' ? routeParams.id : ''
  }
  if (param === 'task_id' && 'task_id' in routeParams) {
    return typeof routeParams.task_id === 'string' ? routeParams.task_id : ''
  }
  return ''
}

function readRequestBody(options: unknown): string | null {
  if (options == null || typeof options !== 'object' || !('body' in options)) {
    return null
  }
  return JSON.stringify(options.body)
}

function renderFloatingChat() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  })
  const rootRoute = createRootRoute({
    component: () => (
      <>
        <FloatingChat />
        <Outlet />
      </>
    ),
  })
  const strategyRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id',
    component: () => <Outlet />,
  })
  const runRoute = createRoute({
    getParentRoute: () => strategyRoute,
    path: 'runs/$taskId',
    component: () => null,
  })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/notes/$noteId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([
      strategyRoute.addChildren([runRoute]),
      noteRoute,
    ]),
    history: createMemoryHistory({
      initialEntries: ['/strategies/demo-strategy-a'],
    }),
  })
  function Wrapper({ children }: { children: React.ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }
  render(<RouterProvider router={router} />, { wrapper: Wrapper })
  return router
}

afterEach(() => {
  cleanup()
  closeFloatingChat()
  apiMock.request.mockReset()
})

describe('FloatingChat', () => {
  it('uses the submitted strategy for task and note requests after route navigation', async () => {
    const requests: CapturedRequest[] = []
    let resolveTaskResponse = (): void => {}
    let taskRequestStarted = false
    const taskResponse = new Promise<unknown>((resolve) => {
      resolveTaskResponse = () => {
        resolve({
          phase: 'completed',
          result_text: '確認が完了しました。',
        })
      }
    })

    apiMock.request.mockImplementation(async ({ method, path, options }) => {
      const requestPath = readRequestPath(path, options)
      const body = method === 'post' ? readRequestBody(options) : null
      requests.push({ method, path: requestPath, body })

      if (path === '/api/strategies' && method === 'get') {
        return [
          { id: 'demo-strategy-a', name: '例示戦略 A' },
          { id: 'demo-strategy-b', name: '例示戦略 B' },
        ]
      }
      if (path === '/api/agent-configs' && method === 'get') return []
      if (path === '/api/strategies/{id}/chat' && method === 'post') {
        return { task_id: 'demo-task-a', a2a_task_id: 'demo-a2a-task' }
      }
      if (path === '/api/strategies/{id}/tasks/{task_id}' && method === 'get') {
        taskRequestStarted = true
        return taskResponse
      }
      if (
        path === '/api/strategies/{id}/tasks/{task_id}/notes' &&
        method === 'get'
      ) {
        return []
      }
      return []
    })
    const router = renderFloatingChat()

    fireEvent.click(
      await screen.findByRole('button', { name: 'アナリストを呼ぶ' }),
    )
    fireEvent.change(await screen.findByLabelText('メッセージ入力'), {
      target: { value: '架空銘柄の状況を確認' },
    })
    fireEvent.click(screen.getByRole('button', { name: '送信' }))
    await waitFor(() => {
      if (!taskRequestStarted) {
        throw new Error('task request has not started')
      }
    })

    await router.navigate({
      to: '/strategies/$id',
      params: { id: 'demo-strategy-b' },
    })
    resolveTaskResponse()

    await waitFor(() => {
      expect(readTaskRequests(requests)).toEqual([
        {
          method: 'post',
          path: '/api/strategies/demo-strategy-a/chat',
          body: '{"prompt":"架空銘柄の状況を確認"}',
        },
        {
          method: 'get',
          path: '/api/strategies/demo-strategy-a/tasks/demo-task-a',
          body: null,
        },
        {
          method: 'get',
          path: '/api/strategies/demo-strategy-a/tasks/demo-task-a/notes',
          body: null,
        },
      ])
    })
  })
})
