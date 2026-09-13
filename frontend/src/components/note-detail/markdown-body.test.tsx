import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

afterEach(cleanup)

// RefChip が使う $api.useQuery('/api/refs/resolve') 用のモック。常に未解決を返す
const refResolveMiddleware: Middleware = {
  onRequest({ request }) {
    if (!/\/api\/refs\/resolve(\?|$)/.test(request.url)) return undefined
    return new Response('[]', {
      status: 200,
      headers: { 'content-type': 'application/json' },
    })
  },
}
beforeEach(() => {
  fetchClient.use(refResolveMiddleware)
})
afterEach(() => {
  fetchClient.eject(refResolveMiddleware)
})

// RefChip が $api.useQuery を使うため QueryClientProvider が要る
function renderMarkdown(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  )
}

const GRAPH_DEF: components['schemas']['GraphDef'] = {
  id: 'g1',
  layout: 'flow',
  nodes: [
    { id: 'a', label: '架空商事' },
    { id: 'b', label: '架空物産' },
  ],
  edges: [{ source: 'a', target: 'b' }],
}

describe('MarkdownBody', () => {
  it('renders a gfm table with alignment', () => {
    const src = ['| item | value |', '| --- | ---: |', '| a | 1 |'].join('\n')
    renderMarkdown(<MarkdownBody source={src} />)
    expect(
      screen.getByRole('columnheader', { name: 'item' }),
    ).toBeInTheDocument()
    expect(screen.getByRole('cell', { name: '1' })).toHaveAttribute(
      'align',
      'right',
    )
  })

  it('renders a link that opens in a new tab', () => {
    renderMarkdown(<MarkdownBody source="[記事](https://example.com/a)" />)
    const link = screen.getByRole('link', { name: '記事' })
    expect(link.getAttribute('href')).toBe('https://example.com/a')
    expect(link.getAttribute('target')).toBe('_blank')
    expect(link.getAttribute('rel')).toBe('noopener noreferrer')
  })

  it('renders a nested list', () => {
    const src = '- top\n    - nested\n'
    const { container } = renderMarkdown(<MarkdownBody source={src} />)
    const items = container.querySelectorAll('li')
    expect(items).toHaveLength(2)
    expect(items[0]?.contains(items[1] ?? null)).toBe(true)
  })

  it('distinguishes a fenced code block from inline code', () => {
    const src = 'inline `x` code\n\n```\nline one\n```\n'
    renderMarkdown(<MarkdownBody source={src} />)

    const inline = screen.getByText('x')
    expect(inline.tagName).toBe('CODE')
    expect(inline.closest('pre')).toBeNull()

    const block = screen.getByText('line one')
    expect(block.tagName).toBe('CODE')
    expect(block.closest('pre')).not.toBeNull()
  })

  it('replaces [[stock:xxx]] with a clickable ref chip', async () => {
    const user = userEvent.setup()
    const onRef = vi.fn()
    renderMarkdown(
      <MarkdownBody source="銘柄 [[stock:7203]] 参照" onRef={onRef} />,
    )
    await user.click(screen.getByRole('button', { name: /7203/ }))
    expect(onRef).toHaveBeenCalledWith('stock:7203')
  })

  it('replaces [[anno:xxx]] with a clickable annotation button', async () => {
    const user = userEvent.setup()
    const onAnno = vi.fn()
    renderMarkdown(
      <MarkdownBody source="シグナル [[anno:A2]] 参照" onAnno={onAnno} />,
    )
    await user.click(screen.getByRole('button', { name: /A2/ }))
    expect(onAnno).toHaveBeenCalledWith('A2')
  })

  it('leaves an unknown ref prefix as literal text', () => {
    renderMarkdown(<MarkdownBody source="未知 [[foo:bar]] は素通り" />)
    expect(screen.getByText('未知 [[foo:bar]] は素通り')).toBeInTheDocument()
  })

  it('renders a graph in place of a standalone [[graph:xxx]] token', () => {
    renderMarkdown(
      <MarkdownBody
        source={'業界の説明\n\n[[graph:g1]]\n\n続きの説明'}
        graphs={[GRAPH_DEF]}
      />,
    )
    expect(screen.getByText('業界の説明')).toBeInTheDocument()
    expect(screen.getByText('架空商事')).toBeInTheDocument()
    expect(screen.getByText('架空物産')).toBeInTheDocument()
    expect(screen.getByText('続きの説明')).toBeInTheDocument()
  })

  it('does not nest the graph container inside a <p>', () => {
    const { container } = renderMarkdown(
      <MarkdownBody source="[[graph:g1]]" graphs={[GRAPH_DEF]} />,
    )
    expect(screen.getByText('架空商事').closest('p')).toBeNull()
    expect(container.querySelector('p')).toBeNull()
  })

  it('shows a fallback instead of crashing when the referenced graph id is missing', () => {
    renderMarkdown(
      <MarkdownBody source="[[graph:missing]]" graphs={[GRAPH_DEF]} />,
    )
    expect(screen.getByRole('alert')).toHaveTextContent('missing')
  })

  it('shows a fallback when a graph token exists but graphs itself is omitted', () => {
    renderMarkdown(<MarkdownBody source="[[graph:g1]]" />)
    expect(screen.getByRole('alert')).toHaveTextContent('g1')
  })
})
