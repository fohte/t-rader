import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import type { components } from '#lib/api/schema.gen'
import { resetFetch, stubFetch } from '#test/stub-fetch.test-helper'

type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']

const strategy: Strategy = {
  id: '00000000-0000-0000-0000-000000000001',
  name: '架空戦略 A',
  description: null,
  sort_order: 0,
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}
const selectedStrategy: Strategy = {
  id: '00000000-0000-0000-0000-000000000002',
  name: '架空戦略 B',
  description: null,
  sort_order: 1,
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}
const stock: Stock = {
  id: 'DEMO-A',
  name: '架空商事',
  market: null,
  sector_id: null,
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}
const selectedStock: Stock = {
  id: 'DEMO-B',
  name: '架空工業',
  market: null,
  sector_id: null,
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}

afterEach(() => {
  cleanup()
  resetFetch()
})

describe('TradeFormDialog', () => {
  it('submits the selected side and form values', async () => {
    const requests: Array<{ method: string; path: string; body: unknown }> = []
    stubFetch(async (input, init) => {
      const request =
        input instanceof Request ? input : new Request(input, init)
      if (request.method !== 'POST') return Response.json({})

      requests.push({
        method: request.method,
        path: new URL(request.url).pathname,
        body: await request.clone().json(),
      })
      return Response.json({})
    })

    const { TradeFormDialog } =
      await import('#components/trades/trade-form-dialog')

    const queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    })
    render(
      <QueryClientProvider client={queryClient}>
        <TradeFormDialog
          open
          onOpenChange={() => {}}
          initial={null}
          defaultDate="2099-01-03"
          strategies={[strategy, selectedStrategy]}
          stocks={[stock, selectedStock]}
        />
      </QueryClientProvider>,
    )
    const user = userEvent.setup()

    await user.click(screen.getByRole('combobox', { name: '銘柄' }))
    await user.click(
      await screen.findByRole('option', { name: '架空工業 (DEMO-B)' }),
    )
    await user.click(screen.getByRole('combobox', { name: '戦略' }))
    await user.click(await screen.findByRole('option', { name: '架空戦略 B' }))
    await user.click(screen.getByRole('combobox', { name: '入力ソース' }))
    await user.click(await screen.findByRole('option', { name: 'CSV 取込' }))

    fireEvent.change(screen.getByLabelText('数量 (株)'), {
      target: { value: '20' },
    })
    fireEvent.change(screen.getByLabelText('単価 (¥)'), {
      target: { value: '1200' },
    })
    await user.click(screen.getByRole('button', { name: '売り' }))
    await user.click(screen.getByRole('button', { name: '追加' }))

    await waitFor(() => {
      expect(requests).toEqual([
        {
          method: 'POST',
          path: '/api/trades',
          body: {
            strategy_id: selectedStrategy.id,
            symbol: selectedStock.id,
            side: 'sell',
            qty: 20,
            price: 1200,
            fee: 0,
            date: '2099-01-03',
            source: 'csv',
            note: null,
          },
        },
      ])
    })
  }, 20_000)
})
