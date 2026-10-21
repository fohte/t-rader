import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { IndicatorObservationChart } from '#components/indicator-observation-chart'

afterEach(cleanup)

describe('IndicatorObservationChart', () => {
  it('changes the selected observation period', async () => {
    const onPeriodChange = vi.fn()
    const user = userEvent.setup()

    render(
      <IndicatorObservationChart
        observations={[]}
        period="1y"
        onPeriodChange={onPeriodChange}
        isPending={true}
        isError={false}
      />,
    )

    await user.click(screen.getByRole('button', { name: '3M' }))

    expect(onPeriodChange.mock.calls).toEqual([['3m']])
  })
})
