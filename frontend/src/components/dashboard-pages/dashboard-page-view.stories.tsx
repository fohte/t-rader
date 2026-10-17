import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  type DashboardPageState,
  DashboardPageView,
  type DashboardWidgetPageState,
} from '#components/dashboard-pages/dashboard-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

function withDashboardRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/dashboards', '/groups/$axisKey/$groupKey'],
        },
      )}
    />
  )
}

const meta = {
  title: 'DashboardPages/DashboardPageView',
  component: DashboardPageView,
  decorators: [withDashboardRouter],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof DashboardPageView>

export default meta
type Story = StoryObj<typeof meta>

const stockResults: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: { movement: 2.4, size: 40 },
    },
    {
      key: 'stock:DEMO-B',
      name: '架空銘柄 B',
      values: { movement: -1.3, size: 25 },
    },
  ],
}

const groupResults: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'group:sample-axis/sample-group-a',
      name: '分類 A',
      values: { movement: 2.4, size: 40 },
    },
    {
      key: 'group:sample-axis/sample-group-b',
      name: '分類 B',
      values: { movement: -1.3, size: 25 },
    },
  ],
}

type ReadyDashboardPageState = Extract<DashboardPageState, { status: 'ready' }>

const movementList: DashboardWidgetPageState = {
  key: 'movement-list',
  title: '値動きの一覧',
  width: 'half',
  definition: {
    viz: { type: 'toplist', value: 'movement', order: 'desc', limit: 5 },
  },
  isPending: false,
  errorMessage: null,
  result: stockResults,
}

const readyState: ReadyDashboardPageState = {
  status: 'ready',
  title: '市場の動き',
  asOf: '2099-01-05',
  onAsOfChange: () => undefined,
  widgets: [
    movementList,
    {
      key: 'group-map',
      title: '分類ごとの動き',
      width: 'full',
      definition: {
        viz: { type: 'treemap', size: 'size', color: 'movement' },
      },
      isPending: false,
      errorMessage: null,
      result: groupResults,
    },
  ],
}

export const Widgets: Story = {
  name: '基準日を指定して幅の異なるウィジェットを表示します。',
  args: { state: readyState },
}

export const Latest: Story = {
  name: '最新のデータを表示します。',
  args: { state: { ...readyState, asOf: '' } },
}

export const Preview: Story = {
  name: '保存前の設定を一覧へのリンクなしで表示します。',
  args: { state: readyState, showDashboardListLink: false },
}

export const Loading: Story = {
  name: 'ダッシュボードを読み込んでいる状態を表示します。',
  args: { state: { status: 'loading', title: '市場の動き' } },
}

export const Error: Story = {
  name: 'ダッシュボードを取得できない状態を表示します。',
  args: {
    state: {
      status: 'error',
      title: '市場の動き',
      message: 'ダッシュボードを取得できませんでした。',
    },
  },
}

export const NotFound: Story = {
  name: '指定したダッシュボードが見つからない状態を表示します。',
  args: { state: { status: 'not-found', title: 'sample-market' } },
}

export const InvalidConfig: Story = {
  name: 'YAML の検証エラーを表示します。',
  args: {
    state: {
      status: 'invalid',
      title: 'sample-market',
      issues: [
        {
          path: 'widgets[0].viz',
          message:
            'queries と treemap または toplist の viz を含むウィジェット定義が必要です。',
        },
      ],
    },
  },
}

export const SyntaxError: Story = {
  name: 'YAML の構文エラーを表示します。',
  args: {
    state: {
      status: 'invalid',
      title: 'プレビュー',
      issues: [{ path: '$', message: 'YAML の構文を確認してください。' }],
    },
    showDashboardListLink: false,
  },
}

export const WidgetLoading: Story = {
  name: 'ウィジェットのデータを読み込んでいる状態を表示します。',
  args: {
    state: {
      ...readyState,
      widgets: [{ ...movementList, isPending: true, result: null }],
    },
  },
}

export const WidgetError: Story = {
  name: '指定日以前にデータがないウィジェットの状態を表示します。',
  args: {
    state: {
      ...readyState,
      widgets: [
        {
          ...movementList,
          errorMessage:
            '基準日以前のデータがありません。基準日を変更してください。',
          result: null,
        },
      ],
    },
  },
}

export const NoWidgets: Story = {
  name: 'ウィジェットが登録されていない状態を表示します。',
  args: { state: { ...readyState, widgets: [] } },
}
