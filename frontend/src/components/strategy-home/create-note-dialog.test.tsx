import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

function readDialogOpenOutput(
  requestsBeforeOpen: string[],
  requestsAfterOpen: string[],
  heading: string | null,
) {
  return { requestsBeforeOpen, requestsAfterOpen, heading }
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
  })
})
