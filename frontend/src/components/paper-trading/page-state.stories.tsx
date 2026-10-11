import type { Meta, StoryObj } from '@storybook/react-vite'

import { PaperTradingPageStateView } from '#components/paper-trading/page-state'

const meta = {
  title: 'PaperTrading/PageState',
  component: PaperTradingPageStateView,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof PaperTradingPageStateView>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: '読み込み中の画面にスケルトンを表示する。',
  args: { state: 'loading' },
}

export const Error: Story = {
  name: '取得に失敗したときにエラーを表示する。',
  args: { state: 'error' },
}

export const NotFound: Story = {
  name: '口座が見つからないときに案内を表示する。',
  args: { state: 'not-found' },
}
