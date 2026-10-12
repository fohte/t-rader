import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { DashboardWeeklyEvents } from '#components/dashboard/dashboard-weekly-events'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Stock = components['schemas']['Stock']

const stocks: Stock[] = [
  {
    id: '0000',
    name: 'デモ商事',
    market: null,
    sector_id: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]
const events: CalendarEvent[] = [
  {
    kind: 'event',
    event_date: '2026-10-14',
    event_at: null,
    time_of_day: '引け後',
    country: 'JP',
    category: 'earnings',
    title: '四半期決算',
    source: 'fixture',
    external_id: 'fake-event-a',
    stock_id: '0000',
    fiscal_period: null,
  },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/calendar', '/charts/$instrumentId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Dashboard/DashboardWeeklyEvents',
  component: DashboardWeeklyEvents,
  decorators: [withRouter],
  args: { events, stocks, isError: false },
} satisfies Meta<typeof DashboardWeeklyEvents>

export default meta
type Story = StoryObj<typeof meta>

export const Upcoming: Story = {
  name: 'shows upcoming events and links to their stock pages.',
}

export const Empty: Story = {
  name: 'shows an empty state when the week has no events.',
  args: { events: [] },
}

export const Loading: Story = {
  name: 'shows placeholders while events are loading.',
  args: { events: null },
}

export const Error: Story = {
  name: 'shows an error message when events cannot be loaded.',
  args: { events: null, isError: true },
}
