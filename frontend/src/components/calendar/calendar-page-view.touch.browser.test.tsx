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
const EVENT_REFERENCE_DEFAULTS = {
  fiscal_period_type: null,
  has_matching_earnings_summary: false,
  references: [],
} satisfies Pick<
  Extract<CalendarEvent, { kind: 'event' }>,
  'fiscal_period_type' | 'has_matching_earnings_summary' | 'references'
>

interface CdpTargetInfo {
  targetInfo: { targetId: string; type: string; url: string }
}

declare module 'vitest/browser' {
  interface CDPSession {
    on(event: string, listener: (target: CdpTargetInfo) => void): void
    off(event: string, listener: (target: CdpTargetInfo) => void): void
  }
}
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
  expectedResultHref?: string
  expectedResultSearch?: Record<string, unknown>
  hasMatchingSummary?: boolean
  fiscalPeriodType?: string
  fiscalPeriodEnd?: string
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
      '引け後JP決算架空高機能素材複合加工研究製造会社 (0001)実績あり該当期の実績を見る →',
    expectedResultHref:
      '/stocks/0001?stockTab=financials&periodEnd=2099-06-30&periodType=2Q',
    expectedResultSearch: {
      stockTab: 'financials',
      periodEnd: '2099-06-30',
      periodType: '2Q',
    },
    hasMatchingSummary: true,
    fiscalPeriodType: '2Q',
    fiscalPeriodEnd: '2099-06-30',
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
    expectedDetailsText:
      '寄り前US決算架空地域情報通信設備総合開発研究製造会社実績・資料未取得 / 公表未確認',
  },
]

function createEarning(scenario: Scenario): CalendarEvent {
  return {
    ...EVENT_REFERENCE_DEFAULTS,
    kind: 'event',
    source: scenario.source,
    external_id:
      scenario.country === 'JP'
        ? `${scenario.stockId}:2Q:${scenario.fiscalPeriodEnd ?? '2099-06-30'}`
        : `sample-expanded-${scenario.country.toLowerCase()}-earnings`,
    category: 'earnings',
    country: scenario.country,
    title: scenario.title,
    stock_id: scenario.stockId,
    fiscal_period: scenario.fiscalPeriodEnd ?? '2099-01-01',
    fiscal_period_type: scenario.fiscalPeriodType ?? null,
    has_matching_earnings_summary: scenario.hasMatchingSummary ?? false,
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
      today="2099-01-07"
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
    search: router.state.location.search,
  }
}

async function expectNewTab(
  session: ReturnType<typeof cdp>,
  targetUrl: string,
  activateLink: () => Promise<void>,
) {
  await session.send('Target.setDiscoverTargets', { discover: true })

  let targetId: string | undefined
  let resolveOpenedTarget:
    ((targetInfo: CdpTargetInfo['targetInfo']) => void) | undefined
  const openedTarget = new Promise<CdpTargetInfo['targetInfo']>((resolve) => {
    resolveOpenedTarget = resolve
  })
  const onTarget = ({ targetInfo }: CdpTargetInfo) => {
    if (targetInfo.type !== 'page' || targetInfo.url !== targetUrl) return
    targetId = targetInfo.targetId
    resolveOpenedTarget?.(targetInfo)
  }
  session.on('Target.targetCreated', onTarget)
  session.on('Target.targetInfoChanged', onTarget)

  try {
    await activateLink()
    const target = await openedTarget
    targetId = target.targetId
    expect(target.url).toEqual(targetUrl)
  } finally {
    session.off('Target.targetCreated', onTarget)
    session.off('Target.targetInfoChanged', onTarget)
    const cleanupCommands = [
      session.send('Target.setDiscoverTargets', { discover: false }),
    ]
    if (targetId != null) {
      cleanupCommands.push(session.send('Target.closeTarget', { targetId }))
    }
    await Promise.all(cleanupCommands)
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

        if (scenario.expectedResultHref != null) {
          await tapTouchTarget(toggle.selector)
          outputs.push(readInteractionOutput(router))

          await tapLinkOnTouch('該当期の実績を見る →')
          await screen.findByTestId('stock-page')
          outputs.push(readInteractionOutput(router))

          router.history.back()
          await screen.findByRole('heading', { name: 'WEEKLY EVENTS' })
          await screen.findByRole('button', { name: '他 1 社' })
          outputs.push(readInteractionOutput(router))
        }

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
        search: {},
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
          ...(scenario.expectedResultHref == null
            ? []
            : [
                {
                  name: '該当期の実績を見る →',
                  href: scenario.expectedResultHref,
                },
              ]),
        ],
        stockPage: null,
        search: {},
      }

      const expectedInteraction: InteractionOutput[] = [
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
          search: {},
        },
        collapsed,
      ]

      if (
        scenario.expectedResultHref != null &&
        scenario.expectedResultSearch != null
      ) {
        expectedInteraction.push(
          expanded,
          {
            viewport: EXPECTED_MOBILE_VIEWPORT,
            pathname: scenario.expectedPathname,
            expanded: null,
            detailsId: null,
            detailsText: null,
            links: [],
            stockPage: '銘柄ページ',
            search: scenario.expectedResultSearch,
          },
          collapsed,
        )
      }

      expect(interaction).toEqual(expectedInteraction)
    },
  )
})

describe('CalendarPageView indicator references on mobile', () => {
  it('opens an individual publication in a new tab when tapped', async () => {
    const targetUrl = new URL(
      '/__link-fixture__/calendar-indicator-release',
      window.location.origin,
    ).href
    const event: CalendarEvent = {
      ...EVENT_REFERENCE_DEFAULTS,
      kind: 'event',
      source: 'e_stat',
      external_id: 'sample-indicator-event',
      category: 'indicator',
      country: 'JP',
      title: '架空経済指標',
      stock_id: null,
      fiscal_period: null,
      event_date: EVENT_DATE,
      event_at: null,
      time_of_day: null,
      references: [
        {
          kind: 'individual_document',
          source: 'sample_source',
          external_id: 'sample-indicator-release',
          title: '架空の公式発表',
          url: targetUrl,
          target_period_start: null,
          target_period_end: '2098-12-31',
          target_period_type: 'month',
          publication_evidence: '架空の公表記録',
          publication_evidence_url: null,
          checked_at: '2099-01-06T00:00:00Z',
        },
      ],
    }
    const router = createStoryRouter(
      () => (
        <CalendarPageView
          calendar={{ ...WEEK_RANGE, events: [event] }}
          today="2099-01-07"
          weekRange={WEEK_RANGE}
          strategies={[]}
          selectedStrategyId={undefined}
          countryFilter="all"
          isPending={false}
          errorMessage={undefined}
          strategyErrorMessage={undefined}
          expandedOtherEarnings={undefined}
          otherEarningsEvents={undefined}
          isOtherEarningsPending={false}
          otherEarningsErrorMessage={undefined}
          onPreviousWeek={() => {}}
          onNextWeek={() => {}}
          onStrategyChange={() => {}}
          onCountryChange={() => {}}
          onOtherEarningsToggle={() => {}}
        />
      ),
      { paths: ['/calendar'], initialPath: '/calendar' },
    )
    const session = cdp()

    await withMobileViewport(session, async () => {
      render(<RouterProvider router={router} />)
      await screen.findByRole('link', { name: '架空の公式発表 →' })

      await expectNewTab(session, targetUrl, () =>
        tapLinkOnTouch('架空の公式発表 →'),
      )
    })
  })
})
