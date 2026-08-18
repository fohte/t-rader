import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { ChipList } from '#components/strategy-settings/agent-graph/fields/chip-list'

const meta = {
  title: 'StrategySettings/AgentGraph/ChipList',
  component: ChipList,
} satisfies Meta<typeof ChipList>

export default meta
type Story = StoryObj<typeof meta>

function Interactive({
  initial,
  options,
}: {
  initial: string[]
  options: string[]
}) {
  const [values, setValues] = useState(initial)
  return (
    <ChipList
      values={values}
      options={options}
      onAdd={(name) => {
        setValues([...values, name])
      }}
      onRemove={(name) => {
        setValues(values.filter((v) => v !== name))
      }}
      removeAriaLabel={(name) => `"${name}" を外す`}
      addAriaLabel="追加"
    />
  )
}

const OPTIONS = ['alpha', 'beta', 'gamma', 'delta']

export const Selected: Story = {
  args: {
    values: ['alpha', 'beta'],
    options: OPTIONS,
    onAdd: () => {},
    onRemove: () => {},
    removeAriaLabel: (name) => `"${name}" を外す`,
    addAriaLabel: '追加',
  },
  render: () => <Interactive initial={['alpha', 'beta']} options={OPTIONS} />,
}

export const NoCandidates: Story = {
  args: {
    values: OPTIONS,
    options: OPTIONS,
    onAdd: () => {},
    onRemove: () => {},
    removeAriaLabel: (name) => `"${name}" を外す`,
    addAriaLabel: '追加',
  },
  render: () => <Interactive initial={OPTIONS} options={OPTIONS} />,
}

// options に無い値 (API 未取得・削除済みなど) も chip としてそのまま表示され続ける
export const ValueNotInOptions: Story = {
  args: {
    values: ['legacy-value'],
    options: OPTIONS,
    onAdd: () => {},
    onRemove: () => {},
    removeAriaLabel: (name) => `"${name}" を外す`,
    addAriaLabel: '追加',
  },
  render: () => <Interactive initial={['legacy-value']} options={OPTIONS} />,
}
