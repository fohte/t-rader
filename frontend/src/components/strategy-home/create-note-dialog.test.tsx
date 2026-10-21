import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { components } from '#lib/api/schema.gen'

type NoteKind = components['schemas']['NoteKind']

function readDialogOpenOutput(
  requestsBeforeOpen: string[],
  requestsAfterOpen: string[],
  heading: string | null,
) {
  return { requestsBeforeOpen, requestsAfterOpen, heading }
}

function readNoteCreationOutput(
  requests: Array<{ path: string; method: string; body: unknown }>,
  dialogOpen: boolean,
) {
  return { requests, dialogOpen }
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.resetModules()
})

describe('CreateNoteDialog', () => {
  it('opens through its default trigger and loads note kinds on demand', async () => {
    const requestedPaths: string[] = []
    vi.stubGlobal('fetch', (input: RequestInfo | URL, init?: RequestInit) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      requestedPaths.push(new URL(request.url).pathname)
      return Promise.resolve(Response.json([]))
    })

    const { CreateNoteDialog } =
      await import('#components/strategy-home/create-note-dialog')
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    })
    render(
      <QueryClientProvider client={queryClient}>
        <CreateNoteDialog />
      </QueryClientProvider>,
    )
    await act(() => Promise.resolve())
    const requestsBeforeOpen = [...requestedPaths]

    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: '+ 新しいノート' }))
    await screen.findByText('利用できる種別はありません')

    expect(
      readDialogOpenOutput(
        requestsBeforeOpen,
        requestedPaths,
        screen.getByRole('heading', { name: '新しいノートを作る' }).textContent,
      ),
    ).toEqual({
      requestsBeforeOpen: [],
      requestsAfterOpen: ['/api/note-kinds'],
      heading: '新しいノートを作る',
    })
  }, 15_000)

  it('選択した種別と本文を作成 API に送る', async () => {
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

        if (path === '/api/note-kinds') {
          return Response.json([
            {
              key: 'sample-kind',
              display_name: 'サンプル種別',
              requires_approval: false,
              description: null,
              sort_order: 0,
            },
          ])
        }
        if (path === '/api/notes' && method === 'POST') {
          return Response.json(
            {
              id: 'demo-note',
              version_id: 'demo-version',
              version_no: 1,
              is_current: true,
              title: '架空ノート',
              body_md: '架空の本文です。',
              frontmatter_json: {},
              resolved_price_references_json: {},
              graphs_json: [],
              tags: [],
              kind: 'sample-kind',
              status: 'unread',
              trigger: null,
              trigger_label: null,
              created_by_kind: 'human',
              created_at: '2099-01-01T00:00:00Z',
              updated_at: '2099-01-01T00:00:00Z',
            },
            { status: 201 },
          )
        }
        return Response.json([])
      },
    )

    const { CreateNoteDialog } =
      await import('#components/strategy-home/create-note-dialog')
    const { $api } = await import('#lib/api/client')
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: Infinity } },
    })
    const noteKinds: NoteKind[] = [
      {
        key: 'sample-kind',
        display_name: 'サンプル種別',
        requires_approval: false,
        description: null,
        sort_order: 0,
      },
    ]
    queryClient.setQueryData(
      $api.queryOptions('get', '/api/note-kinds').queryKey,
      noteKinds,
    )
    render(
      <QueryClientProvider client={queryClient}>
        <CreateNoteDialog />
      </QueryClientProvider>,
    )

    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: '+ 新しいノート' }))
    await user.click(screen.getByRole('combobox', { name: '種別' }))
    await user.click(
      await screen.findByRole('option', { name: 'サンプル種別' }),
    )
    fireEvent.change(screen.getByLabelText('タイトル *'), {
      target: { value: '架空ノート' },
    })
    fireEvent.change(screen.getByLabelText('本文 (Markdown) *'), {
      target: { value: '架空の本文です。' },
    })
    await user.click(screen.getByRole('button', { name: '作成' }))
    await screen.findByRole('button', { name: '+ 新しいノート' })

    expect(
      readNoteCreationOutput(requests, screen.queryByRole('dialog') != null),
    ).toEqual({
      requests: [
        {
          path: '/api/notes',
          method: 'POST',
          body: {
            title: '架空ノート',
            body_md: '架空の本文です。',
            kind: 'sample-kind',
          },
        },
      ],
      dialogOpen: false,
    })
  }, 15_000)
})
