import { describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import {
  buildStockQuote,
  getStockPageListState,
  resolveStockLookup,
  sortAnnotationsNewestFirst,
  sortNotesNewestFirst,
  sortTradesNewestFirst,
} from '#lib/stock-page-model'

type Bar = components['schemas']['Bar']
type Annotation = components['schemas']['Annotation']
type Note = components['schemas']['Note']
type Stock = components['schemas']['Stock']

const stock: Stock = {
  id: 'demo-code',
  name: 'デモ商事',
  market: null,
  sector_id: null,
  product_category: null,
  created_at: '2030-04-01T00:00:00Z',
  updated_at: '2030-04-01T00:00:00Z',
}

const annotations: Annotation[] = [
  {
    id: 'demo-annotation-period',
    target_kind: 'stock',
    target_symbol: 'demo-code',
    timestamp: '2030-04-03T06:00:00Z',
    timestamp_start: '2030-04-01T06:00:00Z',
    price: null,
    text: '期間アノテーションのデモ用メモです。',
    linked_note_id: null,
    status: 'unread',
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2030-04-03T06:00:00Z',
    updated_at: '2030-04-03T06:00:00Z',
  },
  {
    id: 'demo-annotation-point',
    target_kind: 'stock',
    target_symbol: 'demo-code',
    timestamp: '2030-04-02T06:00:00Z',
    timestamp_start: null,
    price: null,
    text: '単日のデモ用メモです。',
    linked_note_id: null,
    status: 'approved',
    created_by_kind: 'human',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2030-04-02T06:00:00Z',
    updated_at: '2030-04-02T06:00:00Z',
  },
]

const makeNote = (id: string, updatedAt: string): Note => ({
  body_md: 'デモ用ノート本文です。',
  created_at: updatedAt,
  created_by_kind: 'human',
  frontmatter_json: {},
  graphs_json: [],
  id,
  is_current: true,
  resolved_price_references_json: {},
  status: 'approved',
  tags: [],
  title: id,
  updated_at: updatedAt,
  version_id: `version-${id}`,
  version_no: 1,
})

const notes: Note[] = [
  makeNote('demo-note-older', '2030-04-01T06:00:00Z'),
  makeNote('demo-note-newer', '2030-04-03T06:00:00Z'),
]

const bars: Bar[] = [
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-02T06:00:00Z',
    open: 118,
    high: 122,
    low: 117,
    close: 120,
    volume: 1000,
  },
  {
    instrument_id: 'demo-code',
    timeframe: '1d',
    timestamp: '2030-04-03T06:00:00Z',
    open: 121,
    high: 127,
    low: 120,
    close: 125,
    volume: 1300,
  },
]

describe('buildStockQuote', () => {
  it('returns the latest close and its change from the previous daily bar', () => {
    expect(buildStockQuote(bars)).toEqual({
      close: 125,
      change: 5,
      changePercent: 4.17,
      timestamp: '2030-04-03T06:00:00Z',
    })
  })

  it('returns null when no daily bars are available', () => {
    expect(buildStockQuote([])).toBeNull()
  })
})

describe('sortTradesNewestFirst', () => {
  it('orders trades by date from newest to oldest', () => {
    expect(
      sortTradesNewestFirst([
        {
          id: 'demo-trade-older',
          strategy_id: '00000000-0000-0000-0000-000000000001',
          symbol: 'demo-code',
          side: 'buy',
          qty: 10,
          price: 120,
          fee: 0,
          date: '2030-04-01',
          source: 'manual',
          note: null,
          note_count: 0,
          created_at: '2030-04-01T06:00:00Z',
          updated_at: '2030-04-01T06:00:00Z',
        },
        {
          id: 'demo-trade-newer',
          strategy_id: '00000000-0000-0000-0000-000000000001',
          symbol: 'demo-code',
          side: 'sell',
          qty: 5,
          price: 125,
          fee: 0,
          date: '2030-04-03',
          source: 'manual',
          note: null,
          note_count: 0,
          created_at: '2030-04-03T06:00:00Z',
          updated_at: '2030-04-03T06:00:00Z',
        },
      ]),
    ).toEqual([
      {
        id: 'demo-trade-newer',
        strategy_id: '00000000-0000-0000-0000-000000000001',
        symbol: 'demo-code',
        side: 'sell',
        qty: 5,
        price: 125,
        fee: 0,
        date: '2030-04-03',
        source: 'manual',
        note: null,
        note_count: 0,
        created_at: '2030-04-03T06:00:00Z',
        updated_at: '2030-04-03T06:00:00Z',
      },
      {
        id: 'demo-trade-older',
        strategy_id: '00000000-0000-0000-0000-000000000001',
        symbol: 'demo-code',
        side: 'buy',
        qty: 10,
        price: 120,
        fee: 0,
        date: '2030-04-01',
        source: 'manual',
        note: null,
        note_count: 0,
        created_at: '2030-04-01T06:00:00Z',
        updated_at: '2030-04-01T06:00:00Z',
      },
    ])
  })
})

describe('sortAnnotationsNewestFirst', () => {
  it('orders period annotations by their start date and point annotations by date', () => {
    expect(sortAnnotationsNewestFirst(annotations)).toEqual([
      annotations[1],
      annotations[0],
    ])
  })
})

describe('sortNotesNewestFirst', () => {
  it('orders notes by update time from newest to oldest', () => {
    expect(sortNotesNewestFirst(notes).map((note) => note.id)).toEqual([
      'demo-note-newer',
      'demo-note-older',
    ])
  })
})

describe('resolveStockLookup', () => {
  it.each([
    ['found', 200, stock, undefined, { kind: 'found', stock }],
    [
      'not found',
      404,
      undefined,
      { error: 'not found' },
      { kind: 'not-found' },
    ],
    [
      'server error',
      500,
      undefined,
      { error: 'unavailable' },
      { kind: 'error' },
    ],
  ] as const)(
    'classifies a %s stock response',
    (_name, status, responseStock, error, expected) => {
      expect(
        resolveStockLookup({
          status,
          stock: responseStock,
          error,
        }),
      ).toEqual(expected)
    },
  )
})

describe('getStockPageListState', () => {
  it.each([
    ['loading', { isPending: true, isError: false }, 'loading'],
    ['error', { isPending: false, isError: true }, 'error'],
    ['ready', { isPending: false, isError: false }, 'ready'],
  ] as const)('maps a %s query to its list state', (_name, query, expected) => {
    expect(getStockPageListState(query)).toEqual(expected)
  })
})
