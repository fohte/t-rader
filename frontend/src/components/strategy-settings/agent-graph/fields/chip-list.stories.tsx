import type { Meta, StoryObj } from '@storybook/react-vite'

import { ChipList } from '#components/strategy-settings/agent-graph/fields/chip-list'

const meta = {
  title: 'StrategySettings/AgentGraph/ChipList',
  component: ChipList,
} satisfies Meta<typeof ChipList>

export default meta
type Story = StoryObj<typeof meta>

const OPTIONS = ['alpha', 'beta', 'gamma', 'delta']

export const Selected: Story = {
  name: 'shows multiple selected items and additional choices.',
  args: {
    values: ['alpha', 'beta'],
    options: OPTIONS,
    onAdd: () => {},
    onRemove: () => {},
    removeAriaLabel: (name) => `"${name}" を外す`,
    addAriaLabel: '追加',
  },
}

export const NoCandidates: Story = {
  name: 'shows all choices selected with no remaining candidates.',
  args: {
    values: OPTIONS,
    options: OPTIONS,
    onAdd: () => {},
    onRemove: () => {},
    removeAriaLabel: (name) => `"${name}" を外す`,
    addAriaLabel: '追加',
  },
}

export const ValueNotInOptions: Story = {
  name: 'keeps a selected item when it is missing from the available choices.',
  args: {
    values: ['legacy-value'],
    options: OPTIONS,
    onAdd: () => {},
    onRemove: () => {},
    removeAriaLabel: (name) => `"${name}" を外す`,
    addAriaLabel: '追加',
  },
}

export const CandidatesOpen: Story = {
  name: 'shows the remaining choices in the open popup.',
  args: {
    values: ['alpha'],
    options: OPTIONS,
    defaultOpen: true,
    onAdd: () => {},
    onRemove: () => {},
    removeAriaLabel: (name) => `"${name}" を外す`,
    addAriaLabel: '追加',
  },
}
