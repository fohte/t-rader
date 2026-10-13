import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { PortfolioHistoryChart } from '#components/portfolio/portfolio-history-chart'

describe('PortfolioHistoryChart', () => {
  it('notifies when the selected period changes', async () => {
    const user = userEvent.setup()
    const onPeriodChange = vi.fn()

    render(
      <PortfolioHistoryChart
        data={[]}
        period="month"
        status="empty"
        onPeriodChange={onPeriodChange}
      />,
    )

    await user.click(screen.getByRole('button', { name: '週' }))

    expect(onPeriodChange.mock.calls).toEqual([['week']])
  })
})
