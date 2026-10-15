import type { Meta, StoryObj } from '@storybook/react-vite'

import { QuickSwitcherItem } from '#components/strategy-shell/quick-switcher-view'

const meta = {
  title: 'StrategyShell/QuickSwitcherItem',
  component: QuickSwitcherItem,
  args: {
    label: 'デモ商事',
    detail: 'demo-code',
    isSelected: false,
    onMouseMove: () => undefined,
    onSelect: () => undefined,
  },
} satisfies Meta<typeof QuickSwitcherItem>

export default meta
type Story = StoryObj<typeof meta>

export const Unselected: Story = {
  name: 'shows an available result that is not selected.',
}

export const Selected: Story = {
  name: 'highlights the result selected by keyboard navigation.',
  args: { isSelected: true },
}
