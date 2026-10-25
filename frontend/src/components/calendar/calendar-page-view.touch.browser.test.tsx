import '#index.css'

import { Outlet, RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { afterEach, describe, expect, it } from 'vitest'
import { cdp, page } from 'vitest/browser'

import {
  isSameOtherEarningsSelection,
  type OtherEarningsSelection,
} from '#components/calendar/calendar-model'
import { CalendarPageView } from '#components/calendar/calendar-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'
import { touchBrowserTestHelpers } from '#vitest/browser-commands.test-helper'

const {
  EXPECTED_MOBILE_VIEWPORT,
  readMobileViewportState,
  tapLinkOnTouch,
  tapTouchTarget,
  withMobileViewport,
} = touchBrowserTestHelpers

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Scenario = {
  country: 'JP' | 'US'
  source: 'jquants' | 'alpha_vantage'
  title: string
  stockId: string
  timeOfDay: 'pre_market' | 'post_market'
  expectedHref: string
  expectedPathname: string
  expectedLinkName: string
  expectedDetailsText: string
}

const EVENT_DATE = '2099-01-06'
const WEEK_RANGE = { from: '2099-01-05', to: '2099-01-11' }
const SCENARIOS: Scenario[] = [
  {
    country: 'JP',
    source: 'jquants',
    title: '架空高機能素材複合加工研究製造会社',
    stockId: '0001',
    timeOfDay: 'post_market',
    expectedHref: '/stocks/0001',
    expectedPathname: '/stocks/0001',
    expectedLinkName: '架空高機能素材複合加工研究製造会社 (0001)',
    expectedDetailsText:
      '引け後JP決算架空高機能素材複合加工研究製造会社 (0001)',
  },
  {
    country: 'US',
    source: 'alpha_vantage',
    title: '架空地域情報通信設備総合開発研究製造会社',
    stockId: 'US:FICT1',
    timeOfDay: 'pre_market',
    expectedHref: '/stocks/US%3AFICT1',
    expectedPathname: '/stocks/US:FICT1',
    expectedLinkName: '架空地域情報通信設備総合開発研究製造会社',
    expectedDetailsText: '寄り前US決算架空地域情報通信設備総合開発研究製造会社',
  },
]

function createEarning(scenario: Scenario): CalendarEvent {
  return {
    kind: 'event',
    source: scenario.source,
    external_id: `sample-expanded-${scenario.country.toLowerCase()}-earnings`,
    category: 'earnings',
    country: scenario.country,
    title: scenario.title,
    stock_id: scenario.stockId,
    fiscal_period: '2099-01-01',
    event_date: EVENT_DATE,
    event_at: null,
    time_of_day: scenario.timeOfDay,
  }
}

function createSummary(scenario: Scenario): CalendarEvent {
  return {
    kind: 'other_earnings_summary',
    country: scenario.country,
    event_date: EVENT_DATE,
    count: 1,
  }
}

function CalendarRoute({ scenario }: { scenario: Scenario }) {
  const [expanded, setExpanded] = useState<OtherEarningsSelection>()
  const selection = { country: scenario.country, eventDate: EVENT_DATE }
  const isExpanded = isSameOtherEarningsSelection(expanded, selection)

  return (
    <CalendarPageView
      calendar={{ ...WEEK_RANGE, events: [createSummary(scenario)] }}
      weekRange={WEEK_RANGE}
      strategies={[]}
      selectedStrategyId={undefined}
      countryFilter="all"
      isPending={false}
      errorMessage={undefined}
      strategyErrorMessage={undefined}
      expandedOtherEarnings={expanded}
      otherEarningsEvents={isExpanded ? [createEarning(scenario)] : undefined}
      isOtherEarningsPending={false}
      otherEarningsErrorMessage={undefined}
      onPreviousWeek={() => {}}
      onNextWeek={() => {}}
      onStrategyChange={() => {}}
      onCountryChange={() => {}}
      onOtherEarningsToggle={(nextSelection) => {
        setExpanded((current) =>
          isSameOtherEarningsSelection(current, nextSelection)
            ? undefined
            : nextSelection,
        )
      }}
    />
  )
}

function renderScenario(scenario: Scenario) {
  const router = createStoryRouter(() => <Outlet />, {
    paths: [
      {
        path: '/calendar',
        component: () => <CalendarRoute scenario={scenario} />,
      },
      {
        path: '/stocks/$stockId',
        component: () => <main data-testid="stock-page">銘柄ページ</main>,
      },
    ],
    initialPath: '/calendar',
  })

  render(<RouterProvider router={router} />)
  return router
}

function readInteractionOutput(router: ReturnType<typeof createStoryRouter>) {
  const toggle = screen.queryByRole('button', { name: '他 1 社' })
  const detailsId = toggle?.getAttribute('aria-controls') ?? null
  const details = detailsId == null ? null : document.getElementById(detailsId)
  const pathname = decodeURIComponent(router.state.location.pathname)

  return {
    viewport: readMobileViewportState(),
    pathname,
    expanded: toggle?.getAttribute('aria-expanded') ?? null,
    detailsId: details?.id ?? null,
    detailsText: details?.textContent ?? null,
    links: [...(details?.querySelectorAll('a') ?? [])].map((link) => ({
      name: link.textContent,
      href: link.getAttribute('href'),
    })),
    stockPage: screen.queryByTestId('stock-page')?.textContent ?? null,
  }
}

type InteractionOutput = ReturnType<typeof readInteractionOutput>

afterEach(cleanup)

describe('CalendarPageView on mobile', () => {
  it.each(SCENARIOS)(
    'expands, collapses, and navigates to the $country earnings stock',
    async (scenario) => {
      const interaction = await withMobileViewport(cdp(), async () => {
        const router = renderScenario(scenario)
        await screen.findByRole('heading', { name: 'WEEKLY EVENTS' })

        const outputs: InteractionOutput[] = [readInteractionOutput(router)]
        const toggle = page.getByRole('button', { name: '他 1 社' })

        await tapTouchTarget(toggle.selector)
        outputs.push(readInteractionOutput(router))

        await tapTouchTarget(toggle.selector)
        outputs.push(readInteractionOutput(router))

        await tapTouchTarget(toggle.selector)
        outputs.push(readInteractionOutput(router))

        await tapLinkOnTouch(scenario.expectedLinkName)
        await screen.findByTestId('stock-page')
        outputs.push(readInteractionOutput(router))

        router.history.back()
        await screen.findByRole('heading', { name: 'WEEKLY EVENTS' })
        await screen.findByRole('button', { name: '他 1 社' })
        outputs.push(readInteractionOutput(router))

        return outputs
      })

      const collapsed = {
        viewport: EXPECTED_MOBILE_VIEWPORT,
        pathname: '/calendar',
        expanded: 'false',
        detailsId: null,
        detailsText: null,
        links: [],
        stockPage: null,
      }
      const expanded: InteractionOutput = {
        viewport: EXPECTED_MOBILE_VIEWPORT,
        pathname: '/calendar',
        expanded: 'true',
        detailsId: `calendar-other-earnings-${scenario.country}-${EVENT_DATE}`,
        detailsText: scenario.expectedDetailsText,
        links: [
          {
            name: scenario.expectedLinkName,
            href: scenario.expectedHref,
          },
        ],
        stockPage: null,
      }

      expect(interaction).toEqual([
        collapsed,
        expanded,
        collapsed,
        expanded,
        {
          viewport: EXPECTED_MOBILE_VIEWPORT,
          pathname: scenario.expectedPathname,
          expanded: null,
          detailsId: null,
          detailsText: null,
          links: [],
          stockPage: '銘柄ページ',
        },
        collapsed,
      ])
    },
  )
})
