import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CHANGE_REFERENCE_BARS } from '#components/note-detail/change-reference-bars.fixtures'
import { MarkdownBody } from '#components/note-detail/markdown-body'
import { createNoteBodyNavigationHandlers } from '#components/note-detail/note-body-navigation'
import type { components } from '#lib/api/schema.gen'

const { useBarsQuery } = vi.hoisted(() => ({ useBarsQuery: vi.fn() }))

vi.mock('#lib/api/client', () => ({ $api: { useQuery: useBarsQuery } }))
vi.mock('#components/candlestick-chart', () => ({
  CandlestickChart: ({ bars }: { bars: unknown[] }) => (
    <div> {bars.length} bars</div>
  ),
}))

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
})
function QueryClientWrapper({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
}

beforeEach(() => {
  useBarsQuery.mockReturnValue({ data: [], isPending: false, isError: false })
})

afterEach(() => {
  cleanup()
  queryClient.clear()
  useBarsQuery.mockReset()
})

const GRAPH_DEF: components['schemas']['GraphDef'] = {
  id: 'g1',
  layout: 'flow',
  nodes: [
    { id: 'a', label: '架空商事' },
    { id: 'b', label: '架空物産' },
  ],
  edges: [{ source: 'a', target: 'b' }],
}

const GRAPH_WITH_REFS: components['schemas']['GraphDef'] = {
  id: 'g-ref-links',
  layout: 'flow',
  nodes: [
    { id: 'stock-node', label: '架空銘柄', ref: 'stock:demo-code' },
    {
      id: 'indicator-node',
      label: '架空指標',
      ref: 'indicator:demo-indicator',
    },
    {
      id: 'group-node',
      label: '架空グループ',
      ref: 'group:demo-axis/demo-group',
    },
  ],
  edges: [
    { source: 'stock-node', target: 'indicator-node' },
    { source: 'indicator-node', target: 'group-node' },
  ],
}

describe('MarkdownBody', () => {
  it('renders a gfm table with alignment', () => {
    const src = ['| item | value |', '| --- | ---: |', '| a | 1 |'].join('\n')
    render(<MarkdownBody source={src} />)
    expect(
      screen.getByRole('columnheader', { name: 'item' }),
    ).toBeInTheDocument()
    expect(screen.getByRole('cell', { name: '1' })).toHaveAttribute(
      'align',
      'right',
    )
  })

  it('renders a link that opens in a new tab', () => {
    render(<MarkdownBody source="[記事](https://example.com/a)" />)
    const link = screen.getByRole('link', { name: '記事' })
    expect(link.getAttribute('href')).toBe('https://example.com/a')
    expect(link.getAttribute('target')).toBe('_blank')
    expect(link.getAttribute('rel')).toBe('noopener noreferrer')
  })

  it('renders a nested list', () => {
    const src = '- top\n    - nested\n'
    const { container } = render(<MarkdownBody source={src} />)
    const items = container.querySelectorAll('li')
    expect(items).toHaveLength(2)
    expect(items[0]?.contains(items[1] ?? null)).toBe(true)
  })

  it('distinguishes a fenced code block from inline code', () => {
    const src = 'inline `x` code\n\n```\nline one\n```\n'
    render(<MarkdownBody source={src} />)

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
    render(
      <MarkdownBody source="銘柄 [[stock:demo-code]] 参照" onRef={onRef} />,
      {
        wrapper: QueryClientWrapper,
      },
    )
    await user.click(screen.getByRole('button', { name: /demo-code/ }))
    expect(onRef.mock.calls).toEqual([
      ['stock:demo-code', { kind: 'stock', id: 'demo-code', name: null }],
    ])
  })

  it('navigates a stock alias with its resolved id', async () => {
    const user = userEvent.setup()
    const navigate = vi.fn()
    const handlers = createNoteBodyNavigationHandlers(navigate)
    useBarsQuery.mockReturnValueOnce({
      data: [{ kind: 'stock', id: 'demo-code', name: '架空商事' }],
      isPending: false,
      isError: false,
    })
    render(
      <MarkdownBody source="[[stock:demo-alias]]" onRef={handlers.onRef} />,
      { wrapper: QueryClientWrapper },
    )

    await user.click(screen.getByRole('button', { name: /架空商事/ }))

    expect(navigate.mock.calls).toEqual([
      [
        {
          to: '/stocks/$stockId',
          params: { stockId: 'demo-code' },
        },
      ],
    ])
  })

  it.each([
    { kind: 'indicator', token: 'indicator:demo-indicator' },
    { kind: 'group', token: 'group:demo-axis/demo-group' },
  ])(
    'renders a $kind ref as a button when onRef is provided',
    ({ kind, token }) => {
      const onRef = vi.fn()
      render(<MarkdownBody source={`[[${token}]]`} onRef={onRef} />, {
        wrapper: QueryClientWrapper,
      })

      expect(screen.getByRole('button').getAttribute('data-kind')).toBe(kind)
    },
  )

  it('keeps a malformed group ref non-interactive', () => {
    const navigate = vi.fn()
    const handlers = createNoteBodyNavigationHandlers(navigate)
    render(
      <MarkdownBody source="[[group:demo-group]]" onRef={handlers.onRef} />,
      { wrapper: QueryClientWrapper },
    )

    const chip = screen.getByTitle(/^\[\[group:demo-group\]\]/)
    handlers.onRef('group:demo-group', {
      kind: 'group',
      id: 'demo-group',
      name: null,
    })
    fireEvent.click(chip)

    const actual = `${chip.tagName}:${String(navigate.mock.calls.length)}`
    expect(actual).toBe('SPAN:0')
  })

  it('replaces [[anno:xxx]] with a clickable annotation button', async () => {
    const user = userEvent.setup()
    const onAnno = vi.fn()
    render(<MarkdownBody source="シグナル [[anno:A2]] 参照" onAnno={onAnno} />)
    await user.click(screen.getByRole('button', { name: /A2/ }))
    expect(onAnno).toHaveBeenCalledWith('A2')
  })

  it('navigates from note and graph chips to stocks, indicators, groups, and annotations', async () => {
    const user = userEvent.setup()
    const navigate = vi.fn()
    const handlers = createNoteBodyNavigationHandlers(navigate)
    const annotationId = '00000000-0000-0000-0000-000000000301'
    render(
      <MarkdownBody
        source={`本文 [[stock:demo-code]] [[indicator:demo-indicator]] [[group:demo-axis/demo-group]] と [[anno:${annotationId}]]\n\n[[graph:g-ref-links]]`}
        graphs={[GRAPH_WITH_REFS]}
        onRef={handlers.onRef}
        onAnno={handlers.onAnno}
      />,
      { wrapper: QueryClientWrapper },
    )

    const referenceTokens = [
      'stock:demo-code',
      'indicator:demo-indicator',
      'group:demo-axis/demo-group',
    ]
    for (const token of referenceTokens) {
      for (const chip of screen.getAllByTitle(
        new RegExp(`^\\[\\[${token}\\]\\]`),
      )) {
        if (chip.closest('.react-flow') != null) fireEvent.click(chip)
        else await user.click(chip)
      }
    }
    await user.click(screen.getByTitle(`annotation ${annotationId}`))

    expect(navigate.mock.calls).toEqual([
      [{ to: '/stocks/$stockId', params: { stockId: 'demo-code' } }],
      [{ to: '/stocks/$stockId', params: { stockId: 'demo-code' } }],
      [
        {
          to: '/indicators/$indicatorId',
          params: { indicatorId: 'demo-indicator' },
        },
      ],
      [
        {
          to: '/indicators/$indicatorId',
          params: { indicatorId: 'demo-indicator' },
        },
      ],
      [
        {
          to: '/groups/$axisKey/$groupKey',
          params: { axisKey: 'demo-axis', groupKey: 'demo-group' },
        },
      ],
      [
        {
          to: '/groups/$axisKey/$groupKey',
          params: { axisKey: 'demo-axis', groupKey: 'demo-group' },
        },
      ],
      [
        {
          to: '/annotations/$annoId',
          params: { annoId: annotationId },
        },
      ],
    ])
  })

  it('renders stock, indicator, and group refs in a graph as buttons', () => {
    const onRef = vi.fn()
    const { container } = render(
      <MarkdownBody
        source="[[graph:g-ref-links]]"
        graphs={[GRAPH_WITH_REFS]}
        onRef={onRef}
      />,
      { wrapper: QueryClientWrapper },
    )

    const graphChips = Array.from(
      container.querySelectorAll('[data-kind]'),
    ).map((chip) => [chip.getAttribute('data-kind'), chip.tagName])
    expect(graphChips).toEqual([
      ['stock', 'BUTTON'],
      ['indicator', 'BUTTON'],
      ['group', 'BUTTON'],
    ])
  })

  it('leaves an unknown ref prefix as literal text', () => {
    render(<MarkdownBody source="未知 [[foo:bar]] は素通り" />)
    expect(screen.getByText('未知 [[foo:bar]] は素通り')).toBeInTheDocument()
  })

  it('renders resolved values inline and appends the change figure', () => {
    useBarsQuery.mockReturnValue({
      data: CHANGE_REFERENCE_BARS,
      isPending: false,
      isError: false,
    })
    const { container } = render(
      <MarkdownBody
        source="終値 [[price:fictional-code@2030-01-02:close]]、変化 [[change:fictional-code@2030-01-02..2030-01-03:close]]、未解決 [[price:fictional-code@2030-01-04:close]]"
        resolvedPriceReferences={{
          '[[price:fictional-code@2030-01-02:close]]': {
            value: 1234.5,
          },
          '[[change:fictional-code@2030-01-02..2030-01-03:close]]': {
            value: -14.910858995137763,
          },
        }}
      />,
      { wrapper: QueryClientWrapper },
    )

    expect(container.textContent).toBe(
      `終値 1,234.5、変化 -14.91%、未解決 [[price:fictional-code@2030-01-04:close]]
fictional-code · 2030-01-02 – 2030-01-03 2 bars`,
    )
  })

  it('renders valuation and dated indicator citations as numbers while keeping an undated indicator link', () => {
    const valuationToken = '[[valuation:fictional-code@2030-01-02:per]]'
    const observationToken = '[[indicator:demo-indicator@2030-01-02]]'
    const evaluationToken =
      '[[eval:00000000-0000-0000-0000-000000000101:result]]'
    const { container } = render(
      <MarkdownBody
        source={`PER ${valuationToken}、FX ${observationToken}、結果 ${evaluationToken}、参照 [[indicator:demo-indicator]]`}
        resolvedPriceReferences={{
          [valuationToken]: { value: 27.5 },
          [observationToken]: { value: 160.25 },
          [evaluationToken]: { value: 256.75 },
        }}
        onRef={vi.fn()}
      />,
      { wrapper: QueryClientWrapper },
    )
    expect(container.textContent).toBe(
      'PER 27.5、FX 160.25、結果 256.75、参照 指標demo-indicator',
    )
  })

  it('requests daily bars for the change reference instrument and date range', () => {
    render(
      <MarkdownBody source="[[change:US:FICTIONAL-A@2030-01-02..2030-01-03:close]]" />,
      { wrapper: QueryClientWrapper },
    )

    expect(useBarsQuery.mock.calls).toEqual([
      [
        'get',
        '/api/bars',
        {
          params: {
            query: {
              instrument_id: 'US:FICTIONAL-A',
              timeframe: '1d',
              from: '2030-01-02',
              to: '2030-01-03',
            },
          },
        },
      ],
    ])
  })

  it('keeps an unresolved change token visible when its bars cannot be loaded', () => {
    useBarsQuery.mockReturnValue({
      data: undefined,
      isPending: false,
      isError: true,
    })
    const token = '[[change:US:FICTIONAL-A@2030-01-02..2030-01-03:close]]'
    const { container } = render(<MarkdownBody source={token} />, {
      wrapper: QueryClientWrapper,
    })

    expect(container.textContent).toBe(
      '[[change:US:FICTIONAL-A@2030-01-02..2030-01-03:close]]\nUS:FICTIONAL-A · 2030-01-02 – 2030-01-03区間のチャートを表示できません',
    )
  })

  it.each([
    { name: 'missing entry', references: {} },
    {
      name: 'non-numeric value',
      references: {
        '[[price:fictional-code@2030-01-02:close]]': { value: '1234.5' },
      },
    },
  ])(
    'leaves an unresolved price reference visible ($name)',
    ({ references }) => {
      const { container } = render(
        <MarkdownBody
          source="価格 [[price:fictional-code@2030-01-02:close]]"
          resolvedPriceReferences={references}
        />,
      )

      expect(container.textContent).toBe(
        '価格 [[price:fictional-code@2030-01-02:close]]',
      )
    },
  )

  it('renders a graph in place of a standalone [[graph:xxx]] token', () => {
    render(
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
    const { container } = render(
      <MarkdownBody source="[[graph:g1]]" graphs={[GRAPH_DEF]} />,
    )
    expect(screen.getByText('架空商事').closest('p')).toBeNull()
    expect(container.querySelector('p')).toBeNull()
  })

  it('shows a fallback instead of crashing when the referenced graph id is missing', () => {
    render(<MarkdownBody source="[[graph:missing]]" graphs={[GRAPH_DEF]} />)
    expect(screen.getByRole('alert')).toHaveTextContent('missing')
  })

  it('shows a fallback when a graph token exists but graphs itself is omitted', () => {
    render(<MarkdownBody source="[[graph:g1]]" />)
    expect(screen.getByRole('alert')).toHaveTextContent('g1')
  })
})
