import type { Meta, StoryObj } from '@storybook/react-vite'

import { StockPageGroups } from '#components/stock-page/stock-page-groups'
import type { components } from '#lib/api/schema.gen'

type GroupMembership = components['schemas']['StockGroupMembershipResponse']

const memberships: GroupMembership[] = [
  {
    axis: { key: 'demo-axis', name: 'デモ軸' },
    group: { key: 'demo-group', name: 'デモ群' },
  },
  {
    axis: { key: 'sample-axis', name: 'サンプル軸' },
    group: { key: 'sample-group', name: 'サンプル群' },
  },
]

const meta = {
  title: 'StockPage/StockPageGroups',
  component: StockPageGroups,
  args: { memberships, state: 'ready' },
} satisfies Meta<typeof StockPageGroups>

export default meta
type Story = StoryObj<typeof meta>

export const WithGroups: Story = {
  name: '所属するグループを軸名とグループ名で表示する。',
}

export const WithoutGroups: Story = {
  name: '所属するグループがない状態を表示する。',
  args: { memberships: [] },
}

export const Loading: Story = {
  name: '所属グループを読み込んでいる状態を表示する。',
  args: { state: 'loading' },
}

export const Error: Story = {
  name: '所属グループを取得できなかった状態を表示する。',
  args: { state: 'error' },
}
