import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { NoteVersionBodyComments } from '#components/note-detail/note-version-body-comments'

type CreateCommentRequest = {
  body: Record<string, unknown>
}

const { useQuery, useMutation, createMutation, resolveMutation } = vi.hoisted(
  () => ({
    useQuery: vi.fn(),
    useMutation: vi.fn(),
    createMutation: vi.fn<(request: CreateCommentRequest) => void>(),
    resolveMutation: {
      mutate: vi.fn(),
      isPending: false,
      isError: false,
      variables: undefined,
    },
  }),
)

vi.mock('#lib/api/client', () => ({
  $api: {
    useQuery,
    useMutation,
    queryOptions: vi.fn(),
  },
}))

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
})

beforeEach(() => {
  useQuery.mockReturnValue({ data: [], isPending: false, isError: false })
  useMutation.mockImplementation((_method: string, path: string) =>
    path === '/api/comments/{id}'
      ? resolveMutation
      : {
          mutate: createMutation,
          reset: vi.fn(),
          isPending: false,
          isError: false,
          variables: undefined,
        },
  )
})

afterEach(() => {
  cleanup()
  queryClient.clear()
  useQuery.mockReset()
  useMutation.mockReset()
  createMutation.mockReset()
  resolveMutation.mutate.mockReset()
})

function renderNoteBodyComments(source: string) {
  return render(
    <QueryClientProvider client={queryClient}>
      <NoteVersionBodyComments versionId="sample-version" source={source} />
    </QueryClientProvider>,
  )
}

describe('NoteVersionBodyComments', () => {
  it('sends the selected quote and source range to the current version', async () => {
    const user = userEvent.setup()
    const source = '# サンプル見出し\n\n対象の段落です。\n次の行です。'
    renderNoteBodyComments(source)

    const paragraph = screen.getByText(/対象の段落です。/)
    const textNode = paragraph.firstChild
    if (textNode == null) {
      throw new Error('本文の段落にテキストがありません')
    }
    const range = document.createRange()
    range.setStart(textNode, 0)
    range.setEnd(textNode, '対象の段落です。'.length)
    Object.defineProperty(range, 'getBoundingClientRect', {
      value: () => new DOMRect(20, 100, 140, 20),
    })
    const selection = window.getSelection()
    selection?.removeAllRanges()
    selection?.addRange(range)
    fireEvent.mouseUp(paragraph)
    fireEvent(document, new Event('selectionchange'))

    await user.click(
      screen.getByRole('button', { name: '選択した本文にコメント' }),
    )
    await user.type(
      await screen.findByRole('textbox', { name: '行コメント' }),
      '根拠を補足してください。',
    )
    await user.click(screen.getByRole('button', { name: '送信' }))

    expect(createMutation.mock.calls.map(([request]) => request.body)).toEqual([
      {
        target_kind: 'note_version',
        target_id: 'sample-version',
        body: '根拠を補足してください。',
        anchor_text: '対象の段落です。',
        anchor_side: 'new',
        start_line: 3,
        end_line: 4,
      },
    ])
  })

  it('sends the union of gutter rows selected with Shift-click', async () => {
    const user = userEvent.setup()
    const source = '# サンプル見出し\n\n最初の段落\n\n次の段落'
    renderNoteBodyComments(source)

    await user.click(screen.getByRole('button', { name: '行 3 にコメント' }))
    const secondGutter = screen.getByRole('button', {
      name: '行 5 にコメント',
    })
    fireEvent.pointerDown(secondGutter)
    fireEvent.pointerUp(secondGutter)
    fireEvent.click(secondGutter, { shiftKey: true })
    await user.type(
      screen.getByRole('textbox', { name: '行コメント' }),
      '段落をまとめて確認してください。',
    )
    await user.click(screen.getByRole('button', { name: '送信' }))

    expect(createMutation.mock.calls.map(([request]) => request.body)).toEqual([
      {
        target_kind: 'note_version',
        target_id: 'sample-version',
        body: '段落をまとめて確認してください。',
        anchor_text: '最初の段落\n\n次の段落',
        anchor_side: 'new',
        start_line: 3,
        end_line: 5,
      },
    ])
  })

  it('sends the source range dragged across body blocks', async () => {
    const user = userEvent.setup()
    const source = '# サンプル見出し\n\n最初の段落\n\n次の段落'
    renderNoteBodyComments(source)

    fireEvent.pointerDown(
      screen.getByRole('button', { name: '行 3 にコメント' }),
    )
    fireEvent.pointerMove(screen.getByText('次の段落'))
    fireEvent.pointerUp(screen.getByText('次の段落'))
    await user.type(
      screen.getByRole('textbox', { name: '行コメント' }),
      '範囲をまとめて確認してください。',
    )
    await user.click(screen.getByRole('button', { name: '送信' }))

    expect(createMutation.mock.calls.map(([request]) => request.body)).toEqual([
      {
        target_kind: 'note_version',
        target_id: 'sample-version',
        body: '範囲をまとめて確認してください。',
        anchor_text: '最初の段落\n\n次の段落',
        anchor_side: 'new',
        start_line: 3,
        end_line: 5,
      },
    ])
  })
})
