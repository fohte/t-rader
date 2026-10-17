import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import {
  type MarkdownBodyWidgetState,
  MarkdownBodyWidgetView,
} from '#components/note-detail/markdown-body-widget-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const toplistResult: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'group:demo-axis/demo-group-a',
      name: 'デモグループ A',
      values: { daily: 2.4 },
    },
    {
      key: 'group:demo-axis/demo-group-b',
      name: 'デモグループ B',
      values: { daily: -1.8 },
    },
  ],
}

const treemapResult: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'group:demo-axis/demo-group-a',
      name: 'デモグループ A',
      values: { size: 48, change: 2.4 },
    },
    {
      key: 'group:demo-axis/demo-group-b',
      name: 'デモグループ B',
      values: { size: 30, change: -1.8 },
    },
  ],
}

function withGroupRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/groups/$axisKey/$groupKey'] },
      )}
    />
  )
}

const meta = {
  title: 'NoteDetail/MarkdownBodyWidgetView',
  component: MarkdownBodyWidgetView,
  decorators: [withGroupRouter],
  parameters: { layout: 'padded' },
  args: {
    state: {
      kind: 'ready',
      result: toplistResult,
      viz: { type: 'toplist', value: 'daily', order: 'desc', limit: 5 },
    } satisfies MarkdownBodyWidgetState,
  },
} satisfies Meta<typeof MarkdownBodyWidgetView>

export default meta
type Story = StoryObj<typeof meta>

export const Toplist: Story = {
  name: '基準日とともにグループ別の順位を表示する。',
}

export const Treemap: Story = {
  name: '基準日とともにグループ別の treemap を表示する。',
  args: {
    state: {
      kind: 'ready',
      result: treemapResult,
      viz: { type: 'treemap', size: 'size', color: 'change' },
    },
  },
}

export const Loading: Story = {
  name: 'ウィジェットの計算中に読み込み状態を表示する。',
  args: { state: { kind: 'loading', asOf: '2099-01-05' } },
}

export const InvalidYaml: Story = {
  name: 'YAML の解析に失敗した理由を表示する。',
  args: {
    state: {
      kind: 'definition-error',
      message: 'YAML を解析できません: 不正な構文',
    },
  },
}

export const ValidationError: Story = {
  name: '定義の検証エラーと基準日を表示する。',
  args: {
    state: {
      kind: 'definition-error',
      asOf: '2099-01-05',
      message: 'queries.daily.agg: 集計方法を指定してください。',
    },
  },
}

export const DataError: Story = {
  name: '基準日のデータを取得できない場合のエラーを表示する。',
  args: {
    state: {
      kind: 'data-error',
      asOf: '2099-01-05',
      message: '指定した日のデータがありません。',
    },
  },
}

export const Empty: Story = {
  name: '対象データがない場合に空の順位表を表示する。',
  args: {
    state: {
      kind: 'ready',
      result: { as_of: '2099-01-05', rows: [] },
      viz: { type: 'toplist', value: 'daily', order: 'desc', limit: 5 },
    },
  },
}
