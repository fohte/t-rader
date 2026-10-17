import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  type DashboardListPageState,
  DashboardListPageView,
} from '#components/dashboard-pages/dashboard-list-page-view'
import { createStoryRouter } from '#storybook/story-router'

function withDashboardRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/dashboards/$key'],
        },
      )}
    />
  )
}

const meta = {
  title: 'DashboardPages/DashboardListPageView',
  component: DashboardListPageView,
  decorators: [withDashboardRouter],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof DashboardListPageView>

export default meta
type Story = StoryObj<typeof meta>

const dashboards = [
  { key: 'sample-market', title: '市場の動き' },
  { key: 'sample-groups', title: '分類ごとの比較' },
]

export const SavedDashboards: Story = {
  name: '保存済みのダッシュボードを一覧で表示します。',
  args: { state: { status: 'ready', dashboards } },
}

export const Empty: Story = {
  name: '保存済みのダッシュボードがない状態を表示します。',
  args: { state: { status: 'ready', dashboards: [] } },
}

export const Loading: Story = {
  name: 'ダッシュボードの一覧を読み込んでいる状態を表示します。',
  args: { state: { status: 'loading' } satisfies DashboardListPageState },
}

export const Error: Story = {
  name: 'ダッシュボードの一覧を取得できない状態を表示します。',
  args: {
    state: {
      status: 'error',
      message: 'ダッシュボードの一覧を取得できませんでした。',
    },
  },
}
