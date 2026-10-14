import type { Meta, StoryObj } from '@storybook/react-vite'

import { StockPageState } from '#components/stock-page/stock-page-state'

const meta = {
  title: 'StockPage/StockPageState',
  component: StockPageState,
  args: { state: 'loading' },
} satisfies Meta<typeof StockPageState>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: '銘柄ページの読み込み中を表示する。',
}

export const NotFound: Story = {
  name: '存在しない銘柄コードの案内を表示する。',
  args: { state: 'not-found' },
}

export const Error: Story = {
  name: '銘柄ページの取得エラーを表示する。',
  args: { state: 'error' },
}
