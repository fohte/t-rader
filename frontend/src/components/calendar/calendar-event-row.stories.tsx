import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { CalendarEventRow } from '#components/calendar/calendar-event-row'
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
