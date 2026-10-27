import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { CalendarEventRow } from '#components/calendar/calendar-event-row'
import type {
  CalendarEarningsStatus,
  CalendarIndicatorStatus,
} from '#components/calendar/calendar-model'
import { createStoryRouter } from '#storybook/story-router'

function createCalendarRouter(component: () => React.ReactNode) {
  return createStoryRouter(component, {
    paths: ['/calendar', '/stocks/$stockId'],
    initialPath: '/calendar',
  })
}

const meta = {
  title: 'Calendar/CalendarEventRow',
  component: CalendarEventRow,
  parameters: { layout: 'fullscreen' },
  args: {
    time: '引け後',
    country: 'JP',
    category: '決算',
    title: '架空工業 (0000)',
    stockId: '0000',
    emphasized: false,
    target: true,
    muted: false,
  },
  render: (args) => (
    <RouterProvider
      router={createCalendarRouter(() => (
        <div className="space-y-1">
          <CalendarEventRow {...args} />
          {args.titleAction?.expanded === true &&
            args.titleAction.controlsId != null && (
              <div
                id={args.titleAction.controlsId}
                className="border-l border-border px-3 py-2 text-muted-foreground"
              >
                追加の決算を表示しています。
              </div>
            )}
        </div>
      ))}
    />
  ),
} satisfies Meta<typeof CalendarEventRow>

export default meta
type Story = StoryObj<typeof meta>

const resultReference: CalendarEarningsStatus['references'][number] = {
  key: 'earnings_summary:sample_source:sample-summary',
  title: '架空の財務要約',
  source: 'sample_source',
  checkedAt: '2030-01-01T00:00:00Z',
  href: 'https://example.invalid/sample-summary',
  publicationEvidence: 'sample-release-record',
  publicationEvidenceHref: 'https://example.invalid/sample-evidence',
}

const indicatorDocumentStatus: CalendarIndicatorStatus = {
  label: '個別資料あり',
  references: [
    {
      key: 'individual_document:sample_source:sample-release',
      title: '架空の経済指標発表',
      source: 'sample_source',
      checkedAt: '2030-01-01T00:00:00Z',
      targetPeriodEnd: '2029-12-31',
      targetPeriodType: '月次',
      href: 'https://example.invalid/sample-release',
      publicationEvidence: '架空の公表記録',
      publicationEvidenceHref: 'https://example.invalid/sample-evidence',
    },
  ],
}

const indicatorHalfYearStatus: CalendarIndicatorStatus = {
  label: '個別資料あり',
  references: [
    {
      key: 'individual_document:sample_source:sample-half-year-release',
      title: '架空の半期指標資料',
      source: 'sample_source',
      checkedAt: '2030-01-01T00:00:00Z',
      targetPeriodStart: '2029-01-01',
      targetPeriodEnd: '2029-06-30',
      targetPeriodType: '半期',
      href: 'https://example.invalid/sample-half-year-release',
      publicationEvidence: '架空の公表記録',
    },
  ],
}

const indicatorUnknownPeriodTypeStatus: CalendarIndicatorStatus = {
  label: '個別資料あり',
  references: [
    {
      key: 'individual_document:sample_source:sample-unknown-period-release',
      title: '架空の経済指標発表',
      source: 'sample_source',
      checkedAt: '2030-01-01T00:00:00Z',
      targetPeriodEnd: '2029-12-31',
      href: 'https://example.invalid/sample-unknown-period-release',
      publicationEvidence: '架空の公表記録',
      publicationEvidenceHref:
        'https://example.invalid/sample-unknown-period-evidence',
    },
  ],
}

const indicatorListingStatus: CalendarIndicatorStatus = {
  label: '公式一覧 / 個別資料未確認',
  references: [
    {
      key: 'official_listing:sample_source:sample-listing',
      title: '架空の公式発表一覧',
      source: 'sample_source',
      checkedAt: '2030-01-01T00:00:00Z',
      href: 'https://example.invalid/sample-listing',
    },
  ],
}

const indicatorScheduledStatus: CalendarIndicatorStatus = {
  label: '発表予定',
  references: [],
}

const indicatorScheduledWithListingStatus: CalendarIndicatorStatus = {
  label: '発表予定',
  references: indicatorListingStatus.references,
}

const indicatorDestinationUnconfirmedStatus: CalendarIndicatorStatus = {
  label: '遷移先未確認 / 非リンク',
  references: [],
}

const mobileCalendarViewport = {
  globals: { viewport: { value: 'mobileCalendarIndicator' } },
  parameters: {
    viewport: {
      options: {
        mobileCalendarIndicator: {
          name: 'Calendar indicator mobile',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: { viewport: { width: 390, height: 700 } },
  },
}

export const DomesticTargetEarnings: Story = {
  name: '追跡対象の国内決算をチャートへのリンクで表示します。',
}

export const OverseasPreMarketEarnings: Story = {
  name: '海外決算を銘柄ページへのリンクで表示します。',
  args: {
    time: '寄り前',
    country: 'US',
    title: '架空電子',
    stockId: 'US:FICT1',
    target: false,
  },
}

export const EmphasizedEvent: Story = {
  name: '重要な経済イベントを強調して表示します。',
  args: {
    time: '09:00',
    country: 'US',
    category: '指標',
    title: '架空物価指標',
    stockId: undefined,
    emphasized: true,
    target: false,
    indicatorStatus: indicatorDestinationUnconfirmedStatus,
  },
}

export const OtherEarningsSummary: Story = {
  name: '他社決算の一覧を閉じた状態で表示します。',
  args: {
    time: '',
    title: '他 2 社',
    stockId: undefined,
    target: false,
    muted: true,
    titleAction: { expanded: false, onClick: () => {} },
  },
}

export const OtherEarningsSummaryExpanded: Story = {
  name: '他社決算の一覧を開いた状態で表示します。',
  args: {
    time: '',
    title: '他 2 社',
    stockId: undefined,
    target: false,
    muted: true,
    titleAction: {
      expanded: true,
      controlsId: 'calendar-other-earnings-details',
      onClick: () => {},
    },
  },
}

export const ResultAvailable: Story = {
  name: '対象期の実績と取得した参照先を表示します。',
  args: {
    earningsStatus: {
      label: '実績あり',
      internalTarget: {
        stockId: '0000',
        periodEnd: '2030-06-30',
        periodType: '2Q',
      },
      references: [resultReference],
    },
  },
}

export const DocumentAvailableWithoutSummary: Story = {
  name: '資料が見つかり実績サマリーは未取得の状態を表示します。',
  args: {
    earningsStatus: {
      label: '資料あり / 実績未取得',
      references: [
        {
          ...resultReference,
          key: 'individual_document:sample_source:sample-document',
          title: '架空の決算資料',
          publicationEvidence: 'sample-release-record',
        },
      ],
    },
  },
}

export const UsSecFilingAvailable: Story = {
  name: '米国決算の期末日が一致した 10-Q を資料リンクで表示します。',
  args: {
    country: 'US',
    title: '架空電子',
    stockId: 'US:FICT1',
    earningsStatus: {
      label: '資料あり / 実績未取得',
      references: [
        {
          key: 'individual_document:sec:0900000001-99-000001',
          title: 'SEC Form 10-Q',
          source: 'sec',
          checkedAt: '2030-01-01T00:00:00Z',
          targetPeriodEnd: '2030-06-30',
          targetPeriodType: '10-Q',
          href: 'https://example.invalid/report.htm',
          publicationEvidence: 'SEC submissions record',
        },
      ],
    },
  },
}

export const UsSecFilingUnconfirmed: Story = {
  name: '米国決算の資料を未確認のまま表示します。',
  args: {
    country: 'US',
    title: '架空計測',
    stockId: 'US:FICT2',
    earningsStatus: {
      label: '実績・資料未取得 / 公表未確認',
      references: [],
    },
  },
}

export const FutureEarningsWithUnmatchedDocument: Story = {
  name: '未来の決算で期が一致しない資料も発表予定とともに表示します。',
  args: {
    earningsStatus: {
      label: '発表予定',
      references: [
        {
          key: 'individual_document:sample_source:sample-document',
          title: '架空の別期決算資料',
          source: 'sample_source',
          checkedAt: '2030-01-01T00:00:00Z',
          targetPeriodEnd: '2030-06-29',
          targetPeriodType: '2Q',
          href: 'https://example.invalid/sample-document',
          publicationEvidence: 'sample-release-record',
        },
      ],
    },
  },
}

export const OfficialListingNeedsDocumentCheck: Story = {
  name: '公式一覧だけがあり個別資料を未確認の状態を表示します。',
  args: {
    earningsStatus: {
      label: '個別資料未確認',
      references: [
        {
          key: 'official_listing:sample_source:sample-listing',
          title: '架空の公式一覧',
          source: 'sample_source',
          checkedAt: '2030-01-01T00:00:00Z',
        },
      ],
    },
  },
}

export const PublicationUnconfirmed: Story = {
  name: '実績や資料を未取得で公表状況も未確認の状態を表示します。',
  args: {
    earningsStatus: {
      label: '実績・資料未取得 / 公表未確認',
      references: [],
    },
  },
}

export const IndicatorDocumentAvailableMobile: Story = {
  name: '390px 幅で個別資料へのリンクを表示します。',
  ...mobileCalendarViewport,
  args: {
    time: '08:30',
    country: 'JP',
    category: '指標',
    title: '架空経済指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorDocumentStatus,
  },
}

export const IndicatorHalfYearDocumentMobile: Story = {
  name: '390px 幅で半期の対象期間と個別資料へのリンクを表示します。',
  ...mobileCalendarViewport,
  args: {
    time: '08:30',
    country: 'JP',
    category: '指標',
    title: '架空経済指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorHalfYearStatus,
  },
}

export const IndicatorOfficialListingMobile: Story = {
  name: '390px 幅で個別資料未確認の公式一覧を表示します。',
  ...mobileCalendarViewport,
  args: {
    time: '21:30',
    country: 'US',
    category: '指標',
    title: '架空雇用指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorListingStatus,
  },
}

export const IndicatorDestinationUnconfirmedMobile: Story = {
  name: '390px 幅で遷移先が未確認の指標を非リンクで表示します。',
  ...mobileCalendarViewport,
  args: {
    time: '09:00',
    country: 'JP',
    category: '指標',
    title: '架空生産指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorDestinationUnconfirmedStatus,
  },
}

export const IndicatorScheduledWithListingMobile: Story = {
  name: '390px 幅で発表予定と公式一覧へのリンクを表示します。',
  ...mobileCalendarViewport,
  args: {
    time: '09:00',
    country: 'JP',
    category: '指標',
    title: '架空生産指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorScheduledWithListingStatus,
  },
}

export const IndicatorDocumentAvailableDesktop: Story = {
  name: 'デスクトップ幅で個別資料へのリンクを表示します。',
  args: {
    time: '08:30',
    country: 'US',
    category: '指標',
    title: '架空物価指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorDocumentStatus,
  },
}

export const IndicatorHalfYearDocumentDesktop: Story = {
  name: 'デスクトップ幅で半期の対象期間と個別資料へのリンクを表示します。',
  args: {
    time: '08:30',
    country: 'JP',
    category: '指標',
    title: '架空経済指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorHalfYearStatus,
  },
}

export const IndicatorUnknownPeriodTypeMobile: Story = {
  name: '390px 幅で未知の期間種別を表示せず対象日のみを示します。',
  ...mobileCalendarViewport,
  args: {
    time: '08:30',
    country: 'JP',
    category: '指標',
    title: '架空経済指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorUnknownPeriodTypeStatus,
  },
}

export const IndicatorUnknownPeriodTypeDesktop: Story = {
  name: 'デスクトップ幅で未知の期間種別を表示せず対象日のみを示します。',
  args: {
    time: '08:30',
    country: 'JP',
    category: '指標',
    title: '架空経済指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorUnknownPeriodTypeStatus,
  },
}

export const IndicatorOfficialListingDesktop: Story = {
  name: 'デスクトップ幅で個別資料未確認の公式一覧を表示します。',
  args: {
    time: '13:30',
    country: 'JP',
    category: '指標',
    title: '架空受注指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorListingStatus,
  },
}

export const IndicatorDestinationUnconfirmedDesktop: Story = {
  name: 'デスクトップ幅で遷移先が未確認の指標を非リンクで表示します。',
  args: {
    time: '10:00',
    country: 'US',
    category: '指標',
    title: '架空小売指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorDestinationUnconfirmedStatus,
  },
}

export const IndicatorScheduledDesktop: Story = {
  name: 'デスクトップ幅で発表予定の指標をリンクなしで表示します。',
  args: {
    time: '09:00',
    country: 'US',
    category: '指標',
    title: '架空雇用指標',
    stockId: undefined,
    emphasized: false,
    target: false,
    indicatorStatus: indicatorScheduledStatus,
  },
}
