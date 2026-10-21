import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

type Strategy = components['schemas']['Strategy']
type SbiPreviewResponse = components['schemas']['SbiPreviewResponse']

const SECOND_STRATEGY_ID = '00000000-0000-0000-0000-000000000002'
const strategies: Strategy[] = [
  {
    id: '00000000-0000-0000-0000-000000000001',
    name: '架空戦略 A',
    description: null,
    sort_order: 0,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
  {
    id: SECOND_STRATEGY_ID,
    name: '架空戦略 B',
    description: null,
    sort_order: 1,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
]

const preview: SbiPreviewResponse = {
  rows: [
    {
      row_index: 2,
      date: '2099-01-03',
      symbol: 'DEMO-A',
      stock_name: '架空商事',
      side: 'buy',
      qty: 20,
      price: 1200,
      fee: 150,
      is_duplicate: false,
    },
    {
      row_index: 3,
      date: '2099-01-02',
      symbol: 'DEMO-B',
      stock_name: '架空工業',
      side: 'sell',
      qty: 10,
      price: 1400,
      fee: 100,
      is_duplicate: true,
    },
  ],
  issues: [],
}

afterEach(() => {
  cleanup()
  resetFetch()
})

describe('ImportSbiDialog', () => {
  it('keeps the CSV picker contract and submits the reviewed rows', async () => {
    const requests: Array<{
      path: string
      method: string
      contentType: string | null
      body: unknown
    }> = []
    stubFetch(async (input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      const path = new URL(request.url).pathname
      if (path === '/api/imports/sbi/preview') {
        requests.push({
          path,
          method: request.method,
          contentType: request.headers.get('content-type'),
          body: await request.clone().text(),
        })
        return Response.json(preview)
      }
      if (path === '/api/imports/sbi/commit') {
        requests.push({
          path,
          method: request.method,
          contentType: request.headers.get('content-type'),
          body: await request.clone().json(),
        })
        return Response.json({ imported_count: 2, skipped_count: 0 })
      }
      return Response.json({})
    })

    const { ImportSbiDialog } =
      await import('#components/trades/import-sbi-dialog')

    const queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    })
    render(
      <QueryClientProvider client={queryClient}>
        <ImportSbiDialog open onOpenChange={() => {}} strategies={strategies} />
      </QueryClientProvider>,
    )
    const user = userEvent.setup()
    const fileInput =
      screen.getByLabelText<HTMLInputElement>('CSV ファイルを選択')
    const accept = fileInput.accept
    await user.upload(
      fileInput,
      new File(['架空の CSV 内容'], 'trades.csv', { type: 'text/csv' }),
    )
    await screen.findByRole('table')

    await user.click(
      screen.getByRole('combobox', { name: '全行を戦略にまとめて割当' }),
    )
    await user.click(await screen.findByRole('option', { name: '架空戦略 B' }))
    await user.click(screen.getByRole('checkbox', { name: '行 3 を除外' }))
    await user.click(screen.getByRole('button', { name: '取込実行 (2 件)' }))
    await screen.findByText('取込が完了しました。')

    const expectedRequests = [
      {
        path: '/api/imports/sbi/preview',
        method: 'POST',
        contentType: 'text/csv',
        body: '架空の CSV 内容',
      },
      {
        path: '/api/imports/sbi/commit',
        method: 'POST',
        contentType: 'application/json',
        body: {
          rows: [
            {
              strategy_id: SECOND_STRATEGY_ID,
              date: '2099-01-03',
              symbol: 'DEMO-A',
              stock_name: '架空商事',
              side: 'buy',
              qty: 20,
              price: 1200,
              fee: 150,
            },
            {
              strategy_id: SECOND_STRATEGY_ID,
              date: '2099-01-02',
              symbol: 'DEMO-B',
              stock_name: '架空工業',
              side: 'sell',
              qty: 10,
              price: 1400,
              fee: 100,
            },
          ],
        },
      },
    ]

    expect(accept).toBe('.csv,text/csv')
    await waitFor(() => {
      expect(requests).toEqual(expectedRequests)
    })
  }, 20_000)
})
