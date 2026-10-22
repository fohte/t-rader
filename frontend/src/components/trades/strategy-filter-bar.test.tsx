import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { StrategyFilterBar } from '#components/trades/strategy-filter-bar'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type Trade = components['schemas']['TradeListItem']

const strategyId = '00000000-0000-0000-0000-000000000001'
const strategies: Strategy[] = [
  {
    id: strategyId,
    name: '架空戦略 A',
    description: null,
    sort_order: 0,
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
]
const trades: Trade[] = [
  {
    id: '00000000-0000-0000-0000-000000000010',
    strategy_id: strategyId,
    symbol: 'DEMO-A',
    side: 'buy',
    qty: 20,
    price: 1200,
    fee: 0,
    date: '2099-01-03',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2099-01-03T00:00:00Z',
    updated_at: '2099-01-03T00:00:00Z',
  },
]

afterEach(cleanup)

function renderFilterBar({
  onChange = () => {},
  onOnlyUnlinkedChange = () => {},
  onlyUnlinked = false,
}: {
  onChange?: (value: string) => void
  onOnlyUnlinkedChange?: (value: boolean) => void
  onlyUnlinked?: boolean
} = {}) {
  render(
    <StrategyFilterBar
      trades={trades}
      strategies={strategies}
      value="all"
      onChange={onChange}
      unlinkedCount={1}
      onlyUnlinked={onlyUnlinked}
      onOnlyUnlinkedChange={onOnlyUnlinkedChange}
    />,
  )
}

describe('StrategyFilterBar', () => {
  it('sends the selected strategy ID', async () => {
    const onChange = vi.fn<(value: string) => void>()
    renderFilterBar({ onChange })
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: '架空戦略 A (1)' }))

    expect(onChange.mock.calls).toEqual([[strategyId]])
  })

  it('sends the new unlinked filter value', async () => {
    const onOnlyUnlinkedChange = vi.fn<(value: boolean) => void>()
    renderFilterBar({ onOnlyUnlinkedChange, onlyUnlinked: true })
    const user = userEvent.setup()

    await user.click(screen.getByText('未紐付けのみ'))

    expect(onOnlyUnlinkedChange.mock.calls).toEqual([[false]])
  })
})
