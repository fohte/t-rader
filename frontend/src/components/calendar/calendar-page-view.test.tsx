import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CalendarPageView } from '#components/calendar/calendar-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type CalendarPageViewProps = ComponentProps<typeof CalendarPageView>
type CalendarEvent = components['schemas']['CalendarEventResponse']
type Strategy = components['schemas']['Strategy']

afterEach(cleanup)

const WEEK_RANGE = { from: '2099-01-05', to: '2099-01-11' }
const NOOP = (): void => {}
const STRATEGY: Strategy = {
  id: '00000000-0000-0000-0000-000000000001',
  name: '検証用戦略',
  description: null,
  sort_order: 0,
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}

function makeProps(
  overrides: Partial<CalendarPageViewProps> = {},
): CalendarPageViewProps {
  return {
    calendar: { ...WEEK_RANGE, events: [] },
    weekRange: WEEK_RANGE,
    strategies: [],
    selectedStrategyId: undefined,
    countryFilter: 'all',
    isPending: false,
    errorMessage: undefined,
    strategyErrorMessage: undefined,
    onPreviousWeek: NOOP,
    onNextWeek: NOOP,
    onStrategyChange: NOOP,
    onCountryChange: NOOP,
    ...overrides,
  }
}

function renderCalendarPageView(
  props: CalendarPageViewProps,
): Promise<HTMLElement> {
  const router = createStoryRouter(() => <CalendarPageView {...props} />, {
    paths: ['/calendar', '/charts/$instrumentId'],
    initialPath: '/calendar',
  })
  render(<RouterProvider router={router} />)
  return waitFor(() => screen.getByRole('heading', { name: 'WEEKLY EVENTS' }))
}

describe('CalendarPageView', () => {
  it('requests the previous week when its navigation button is selected', async () => {
    const user = userEvent.setup()
    const onPreviousWeek = vi.fn()

    await renderCalendarPageView(makeProps({ onPreviousWeek }))

    await user.click(screen.getByRole('button', { name: '前の週' }))

    expect(onPreviousWeek.mock.calls).toEqual([[]])
  })

  it('requests the next week when its navigation button is selected', async () => {
    const user = userEvent.setup()
    const onNextWeek = vi.fn()

    await renderCalendarPageView(makeProps({ onNextWeek }))

    await user.click(screen.getByRole('button', { name: '次の週' }))

    expect(onNextWeek.mock.calls).toEqual([[]])
  })

  it('reports the selected country filter', async () => {
    const user = userEvent.setup()
    const onCountryChange = vi.fn()

    await renderCalendarPageView(makeProps({ onCountryChange }))

    await user.selectOptions(screen.getByRole('combobox', { name: '国' }), 'US')

    expect(onCountryChange.mock.calls).toEqual([['US']])
  })

  it('reports the selected strategy filter', async () => {
    const user = userEvent.setup()
    const onStrategyChange = vi.fn()

    await renderCalendarPageView(
      makeProps({ strategies: [STRATEGY], onStrategyChange }),
    )

    await user.selectOptions(
      screen.getByRole('combobox', { name: '戦略' }),
      STRATEGY.id,
    )

    expect(onStrategyChange.mock.calls).toEqual([[STRATEGY.id]])
  })

  it('clears the strategy filter when all strategies is selected', async () => {
    const user = userEvent.setup()
    const onStrategyChange = vi.fn()

    await renderCalendarPageView(
      makeProps({
        strategies: [STRATEGY],
        selectedStrategyId: STRATEGY.id,
        onStrategyChange,
      }),
    )

    await user.selectOptions(screen.getByRole('combobox', { name: '戦略' }), '')

    expect(onStrategyChange.mock.calls).toEqual([[undefined]])
  })

  it('shows a strategy list error beside the strategy filter', async () => {
    await renderCalendarPageView(
      makeProps({ strategyErrorMessage: '戦略一覧の取得に失敗しました' }),
    )

    expect(screen.getByRole('alert').textContent).toBe(
      '戦略一覧の取得に失敗しました',
    )
  })

  it('links domestic earnings to the stock chart', async () => {
    const earning: CalendarEvent = {
      kind: 'event',
      source: 'jquants',
      external_id: 'sample-stock-earnings',
      category: 'earnings',
      country: 'JP',
      title: '架空工業',
      stock_id: '0000',
      fiscal_period: '2099-01-01',
      event_date: '2099-01-06',
      event_at: null,
      time_of_day: null,
    }

    await renderCalendarPageView(
      makeProps({ calendar: { ...WEEK_RANGE, events: [earning] } }),
    )

    expect(
      screen
        .getByRole('link', { name: '架空工業 (0000)' })
        .getAttribute('href'),
    ).toBe('/charts/0000')
  })
})
