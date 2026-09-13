import type { Meta, StoryObj } from '@storybook/react-vite'

import { RefChip } from '#components/strategy-shell/ref-chip'
import type { RefResolveStub } from '#lib/refs.test-helper'
import { RefResolveQueryDecorator } from '#storybook/ref-resolve-mock'

// 以下の銘柄名はすべて架空のもの。実在の企業とは無関係
const STUBS: RefResolveStub[] = [
  { kind: 'stock', id: '7203', name: 'アルファ製作所' },
  { kind: 'stock', id: '3436', name: 'ベータマテリアル' },
  { kind: 'indicator', id: 'USDJPY', name: 'USD/JPY' },
  { kind: 'sector', id: '半導体', name: '半導体' },
  { kind: 'theme', id: '円安', name: '円安' },
]

const meta = {
  title: 'StrategyShell/RefChip',
  component: RefChip,
  decorators: [
    (Story) => (
      <RefResolveQueryDecorator stubs={STUBS}>
        <Story />
      </RefResolveQueryDecorator>
    ),
  ],
} satisfies Meta<typeof RefChip>

export default meta
type Story = StoryObj<typeof meta>

export const Stock: Story = {
  args: { token: 'stock:7203' },
}

export const Indicator: Story = {
  args: { token: 'indicator:USDJPY' },
}

export const Sector: Story = {
  args: { token: 'sector:半導体' },
}

export const Theme: Story = {
  args: { token: 'theme:円安' },
}

export const Pill: Story = {
  args: { token: 'stock:3436', pill: true },
}

// 解決 API に一致が無い token。フォールバック (prefix を落とした生の id) 表示になる
export const Unknown: Story = {
  args: { token: 'stock:9999' },
}
