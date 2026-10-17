import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { NoteVersionBodyComments } from '#components/note-detail/note-version-body-comments'
import type { NoteVersionComment } from '#lib/api/note-version-types'

type ApiRequest = {
  method: 'POST' | 'PATCH'
  path: string
  body: Record<string, unknown>
}
type MutationRequest = {
  params?: { path?: { id: string } }
  body: Record<string, unknown>
}

const comment: NoteVersionComment = {
  id: 'sample-comment',
  target_kind: 'note_version',
  target_id: 'sample-version',
  parent_id: null,
  body: '対象を確認してください。',
  author_kind: 'human',
  author_label: 'レビュー担当者',
  resolved: false,
  anchor_text: '対象の本文',
  anchor_side: 'new',
  start_line: 1,
  end_line: 1,
  created_at: '2026-01-12T03:00:00Z',
}

const { useQuery, useMutation, apiRequests, resolveMutation } = vi.hoisted(
  () => ({
    useQuery: vi.fn(),
    useMutation: vi.fn(),
    apiRequests: [] as ApiRequest[],
    resolveMutation: {
      mutate: vi.fn<(request: MutationRequest) => void>(),
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
  apiRequests.length = 0
  resolveMutation.isError = false
  useMutation.mockImplementation((method: string, path: string) =>
    method === 'patch'
      ? {
          ...resolveMutation,
          mutate: (request: MutationRequest) => {
            apiRequests.push({
              method: 'PATCH',
              path: path.replace('{id}', request.params?.path?.id ?? ''),
              body: request.body,
            })
          },
        }
      : {
          mutate: (request: MutationRequest) => {
            apiRequests.push({ method: 'POST', path, body: request.body })
          },
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
  apiRequests.length = 0
  resolveMutation.mutate.mockReset()
  resolveMutation.isError = false
})

function renderNoteBodyComments(source: string) {
  return render(
    <QueryClientProvider client={queryClient}>
      <NoteVersionBodyComments versionId="sample-version" source={source} />
    </QueryClientProvider>,
  )
}

function selectParagraphText(text: string): HTMLElement {
  const paragraph = screen.getByText((content) => content.includes(text))
  const textNode = paragraph.firstChild
  if (textNode == null) {
    throw new Error('本文の段落にテキストがありません')
  }
  const textContent = textNode.textContent ?? ''
  const start = textContent.indexOf(text)
  if (start === -1) throw new Error('選択する文字列が段落にありません')
  const range = document.createRange()
  range.setStart(textNode, start)
  range.setEnd(textNode, start + text.length)
  Object.defineProperty(range, 'getBoundingClientRect', {
    value: () => new DOMRect(20, 100, 140, 20),
  })
  const selection = window.getSelection()
  selection?.removeAllRanges()
  selection?.addRange(range)
  return paragraph
}

describe('NoteVersionBodyComments', () => {
  it('sends the selected quote and source range to the current version', async () => {
    const user = userEvent.setup()
    const source = `# サンプル見出し

対象の段落です。
次の行です。`
    const { container } = renderNoteBodyComments(source)

    const selectedText = '段落です。'
    const paragraph = selectParagraphText(selectedText)
    fireEvent.mouseUp(paragraph)
    fireEvent(document, new Event('selectionchange'))

    await user.click(
      screen.getByRole('button', { name: '選択した本文にコメント' }),
    )
    const commentInput = await screen.findByRole('textbox', {
      name: '選択した箇所へのコメント',
    })
    await user.type(commentInput, '根拠を補足してください。')
    await user.click(screen.getByRole('button', { name: '送信' }))

    const getCommentResult = () => ({
      composerQuote:
        commentInput.parentElement?.querySelector('blockquote')?.textContent ??
        null,
      placeholder: commentInput.getAttribute('placeholder'),
      highlightedText: [
        ...container.querySelectorAll(
          'mark[data-comment-text-highlight="true"]',
        ),
      ].map((mark) => mark.textContent),
      paragraphHasBlockHighlight: paragraph.classList.contains('bg-primary/10'),
      requests: apiRequests,
    })
    expect(getCommentResult()).toEqual({
      composerQuote: null,
      placeholder: '選択した箇所へのコメント',
      highlightedText: [selectedText],
      paragraphHasBlockHighlight: false,
      requests: [
        {
          method: 'POST',
          path: '/api/comments',
          body: {
            target_kind: 'note_version',
            target_id: 'sample-version',
            body: '根拠を補足してください。',
            anchor_text: selectedText,
            anchor_side: 'new',
            start_line: 3,
            end_line: 4,
          },
        },
      ],
    })
  })

  it('shows the comment action for a text selection after touch ends', async () => {
    const user = userEvent.setup()
    const source = `# サンプル見出し

対象のタッチ段落です。`
    renderNoteBodyComments(source)

    const paragraph = selectParagraphText('対象のタッチ段落です。')
    fireEvent.touchEnd(paragraph)

    await user.click(
      screen.getByRole('button', { name: '選択した本文にコメント' }),
    )
    await user.type(
      await screen.findByRole('textbox', {
        name: '選択した箇所へのコメント',
      }),
      'タッチ選択の指摘です。',
    )
    await user.click(screen.getByRole('button', { name: '送信' }))

    expect(apiRequests).toEqual([
      {
        method: 'POST',
        path: '/api/comments',
        body: {
          target_kind: 'note_version',
          target_id: 'sample-version',
          body: 'タッチ選択の指摘です。',
          anchor_text: '対象のタッチ段落です。',
          anchor_side: 'new',
          start_line: 3,
          end_line: 3,
        },
      },
    ])
  })

  it('does not start a comment when a touch pointer taps the gutter', () => {
    const source = '# サンプル見出し\n\n対象の段落です。'
    renderNoteBodyComments(source)

    const gutter = screen.getByRole('button', { name: '行 3 にコメント' })
    fireEvent.pointerDown(gutter, { pointerType: 'touch' })
    fireEvent.pointerUp(gutter, { pointerType: 'touch' })
    fireEvent.click(gutter, { detail: 1 })

    const getTouchGutterResult = () => ({
      label: gutter.getAttribute('aria-label'),
      composerCount: screen.queryAllByRole('textbox', { name: '行コメント' })
        .length,
      requests: apiRequests,
    })
    expect(getTouchGutterResult()).toEqual({
      label: '行 3 にコメント',
      composerCount: 0,
      requests: [],
    })
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
    const lineCommentInput = screen.getByRole('textbox', {
      name: '行コメント',
    })
    await user.type(lineCommentInput, '段落をまとめて確認してください。')
    await user.click(screen.getByRole('button', { name: '送信' }))

    const getLineCommentResult = () => ({
      placeholder: lineCommentInput.getAttribute('placeholder'),
      requests: apiRequests,
    })
    expect(getLineCommentResult()).toEqual({
      placeholder: 'この行へのコメント',
      requests: [
        {
          method: 'POST',
          path: '/api/comments',
          body: {
            target_kind: 'note_version',
            target_id: 'sample-version',
            body: '段落をまとめて確認してください。',
            anchor_text: '最初の段落\n\n次の段落',
            anchor_side: 'new',
            start_line: 3,
            end_line: 5,
          },
        },
      ],
    })
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

    expect(apiRequests).toEqual([
      {
        method: 'POST',
        path: '/api/comments',
        body: {
          target_kind: 'note_version',
          target_id: 'sample-version',
          body: '範囲をまとめて確認してください。',
          anchor_text: '最初の段落\n\n次の段落',
          anchor_side: 'new',
          start_line: 3,
          end_line: 5,
        },
      },
    ])
  })

  it('sends a reply to the selected comment', async () => {
    const user = userEvent.setup()
    useQuery.mockReturnValue({
      data: [comment],
      isPending: false,
      isError: false,
    })
    renderNoteBodyComments('対象の本文')

    await user.click(screen.getByRole('button', { name: '返信' }))
    await user.type(
      screen.getByRole('textbox', { name: '返信' }),
      '該当箇所を確認しました。',
    )
    await user.click(screen.getByRole('button', { name: '送信' }))

    expect(apiRequests).toEqual([
      {
        method: 'POST',
        path: '/api/comments',
        body: {
          target_kind: 'note_version',
          target_id: 'sample-version',
          parent_id: 'sample-comment',
          body: '該当箇所を確認しました。',
        },
      },
    ])
  })

  it('toggles the resolved state of a comment', async () => {
    const user = userEvent.setup()
    useQuery.mockReturnValue({
      data: [comment],
      isPending: false,
      isError: false,
    })
    renderNoteBodyComments('対象の本文')

    await user.click(screen.getByRole('button', { name: '未解決' }))

    expect(apiRequests).toEqual([
      {
        method: 'PATCH',
        path: '/api/comments/sample-comment',
        body: { resolved: true },
      },
    ])
  })

  it('shows an error when resolving a body comment fails', () => {
    resolveMutation.isError = true
    renderNoteBodyComments('対象の本文')

    expect(screen.getByRole('alert').textContent).toBe(
      'コメントの解決状態を更新できませんでした',
    )
  })
})
