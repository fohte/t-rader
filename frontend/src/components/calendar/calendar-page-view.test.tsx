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
const EVENT_REFERENCE_DEFAULTS = {
  fiscal_period_type: null,
  has_matching_earnings_summary: false,
  references: [],
} satisfies Pick<
  Extract<CalendarEvent, { kind: 'event' }>,
  'fiscal_period_type' | 'has_matching_earnings_summary' | 'references'
>

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
    expandedOtherEarnings: undefined,
    otherEarningsEvents: undefined,
    isOtherEarningsPending: false,
    otherEarningsErrorMessage: undefined,
    onPreviousWeek: NOOP,
    onNextWeek: NOOP,
    onStrategyChange: NOOP,
    onCountryChange: NOOP,
    onOtherEarningsToggle: NOOP,
    ...overrides,
  }
}

function renderCalendarPageView(
  props: CalendarPageViewProps,
): Promise<HTMLElement> {
  const router = createStoryRouter(() => <CalendarPageView {...props} />, {
    paths: ['/calendar', '/stocks/$stockId'],
    initialPath: '/calendar',
  })
  render(<RouterProvider router={router} />)
  return waitFor(() => screen.getByRole('heading', { name: 'WEEKLY EVENTS' }))
}

function getEarningsDetailsRelationship(button: HTMLElement) {
  const controlsId = button.getAttribute('aria-controls')
  const details = document.getElementById(controlsId ?? '')

  return {
    expanded: button.getAttribute('aria-expanded'),
    controlsId,
    detailsId: details?.id,
    detailsText: details?.textContent,
  }
}

function getExpandedEarningsOutput(
  button: HTMLElement,
  details: HTMLElement | null,
) {
  return {
    relationship: getEarningsDetailsRelationship(button),
    links: [...(details?.querySelectorAll('a') ?? [])].map((link) => ({
      name: link.textContent,
      href: link.getAttribute('href'),
    })),
  }
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

    await user.click(screen.getByRole('combobox', { name: '国' }))
    await user.click(await screen.findByRole('option', { name: 'US' }))

    expect(onCountryChange.mock.calls).toEqual([['US']])
  })

  it('reports all countries when the country filter is cleared', async () => {
    const user = userEvent.setup()
    const onCountryChange = vi.fn()

    await renderCalendarPageView(
      makeProps({ countryFilter: 'US', onCountryChange }),
    )

    await user.click(screen.getByRole('combobox', { name: '国' }))
    await user.click(await screen.findByRole('option', { name: 'すべての国' }))

    expect(onCountryChange.mock.calls).toEqual([['all']])
  })

  it('reports the selected strategy filter', async () => {
    const user = userEvent.setup()
    const onStrategyChange = vi.fn()

    await renderCalendarPageView(
      makeProps({ strategies: [STRATEGY], onStrategyChange }),
    )

    await user.click(screen.getByRole('combobox', { name: '戦略' }))
    await user.click(await screen.findByRole('option', { name: STRATEGY.name }))

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

    await user.click(screen.getByRole('combobox', { name: '戦略' }))
    await user.click(await screen.findByRole('option', { name: '全戦略' }))

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
      ...EVENT_REFERENCE_DEFAULTS,
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
    ).toBe('/stocks/0000')
  })

  it('links overseas earnings to the stock chart', async () => {
    const earning: CalendarEvent = {
      ...EVENT_REFERENCE_DEFAULTS,
      kind: 'event',
      source: 'alpha_vantage',
      external_id: 'sample-overseas-earnings',
      category: 'earnings',
      country: 'US',
      title: '架空電子',
      stock_id: 'US:FICT1',
      fiscal_period: '2099-01-01',
      event_date: '2099-01-06',
      event_at: null,
      time_of_day: 'pre_market',
    }

    await renderCalendarPageView(
      makeProps({ calendar: { ...WEEK_RANGE, events: [earning] } }),
    )

    expect(
      screen.getByRole('link', { name: '架空電子' }).getAttribute('href'),
    ).toBe('/stocks/US%3AFICT1')
  })

  it('requests the other earnings for the selected country and date', async () => {
    const user = userEvent.setup()
    const onOtherEarningsToggle = vi.fn()
    const summary: CalendarEvent = {
      kind: 'other_earnings_summary',
      country: 'JP',
      event_date: '2099-01-06',
      count: 2,
    }

    await renderCalendarPageView(
      makeProps({
        calendar: { ...WEEK_RANGE, events: [summary] },
        onOtherEarningsToggle,
      }),
    )

    await user.click(screen.getByRole('button', { name: '他 2 社' }))

    expect(onOtherEarningsToggle.mock.calls).toEqual([
      [{ country: 'JP', eventDate: '2099-01-06' }],
    ])
  })

  it('connects the expanded earnings button to its details region', async () => {
    const summary: CalendarEvent = {
      kind: 'other_earnings_summary',
      country: 'JP',
      event_date: '2099-01-06',
      count: 2,
    }

    await renderCalendarPageView(
      makeProps({
        calendar: { ...WEEK_RANGE, events: [summary] },
        expandedOtherEarnings: { country: 'JP', eventDate: '2099-01-06' },
      }),
    )

    const button = screen.getByRole('button', { name: '他 2 社' })
    expect(getEarningsDetailsRelationship(button)).toEqual({
      expanded: 'true',
      controlsId: 'calendar-other-earnings-JP-2099-01-06',
      detailsId: 'calendar-other-earnings-JP-2099-01-06',
      detailsText: '該当する決算はありません。',
    })
  })

  it.each([
    {
      country: 'JP',
      source: 'jquants',
      title: '架空工業',
      stockId: '0000',
      expectedHref: '/stocks/0000',
      timeOfDay: 'post_market',
      expectedLinkName: '架空工業 (0000)',
      expectedDetailsText:
        '引け後JP決算架空工業 (0000)実績・資料未取得 / 公表未確認',
    },
    {
      country: 'US',
      source: 'alpha_vantage',
      title: '架空電子',
      stockId: 'US:FICT1',
      expectedHref: '/stocks/US%3AFICT1',
      timeOfDay: 'pre_market',
      expectedLinkName: '架空電子',
      expectedDetailsText: '寄り前US決算架空電子実績・資料未取得 / 公表未確認',
    },
  ] as const)(
    'links expanded $country earnings to their stock pages',
    async ({
      country,
      source,
      title,
      stockId,
      expectedHref,
      timeOfDay,
      expectedLinkName,
      expectedDetailsText,
    }) => {
      const eventDate = '2099-01-06'
      const summary: CalendarEvent = {
        kind: 'other_earnings_summary',
        country,
        event_date: eventDate,
        count: 1,
      }
      const earning: CalendarEvent = {
        ...EVENT_REFERENCE_DEFAULTS,
        kind: 'event',
        source,
        external_id: `sample-other-${country.toLowerCase()}-earnings`,
        category: 'earnings',
        country,
        title,
        stock_id: stockId,
        fiscal_period: '2099-01-01',
        event_date: eventDate,
        event_at: null,
        time_of_day: timeOfDay,
      }

      await renderCalendarPageView(
        makeProps({
          calendar: { ...WEEK_RANGE, events: [summary] },
          expandedOtherEarnings: { country, eventDate },
          otherEarningsEvents: [earning],
        }),
      )

      const button = screen.getByRole('button', { name: '他 1 社' })
      const detailsId = `calendar-other-earnings-${country}-${eventDate}`
      const details = document.getElementById(detailsId)

      expect(getExpandedEarningsOutput(button, details)).toEqual({
        relationship: {
          expanded: 'true',
          controlsId: detailsId,
          detailsId,
          detailsText: expectedDetailsText,
        },
        links: [{ name: expectedLinkName, href: expectedHref }],
      })
    },
  )
})
