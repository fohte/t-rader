import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { CalendarEventRow } from '#components/calendar/calendar-event-row'
import type { CalendarEarningsStatus } from '#components/calendar/calendar-model'
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
