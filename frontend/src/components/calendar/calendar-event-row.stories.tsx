import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { CalendarEventRow } from '#components/calendar/calendar-event-row'
import { createStoryRouter } from '#storybook/story-router'

function createCalendarRouter(component: () => React.ReactNode) {
  return createStoryRouter(component, {
    paths: ['/calendar', '/charts/$instrumentId'],
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
        <CalendarEventRow {...args} />
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
  name: '海外銘柄の決算時刻が寄り前と分かっています。',
  args: {
    time: '寄り前',
    country: 'US',
    title: '架空電子',
    stockId: undefined,
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
  name: '他社決算の件数を展開ボタンとして表示します。',
  args: {
    time: '',
    title: '他 2 社',
    stockId: undefined,
    target: false,
    muted: true,
    titleAction: { expanded: true, onClick: () => {} },
  },
}
