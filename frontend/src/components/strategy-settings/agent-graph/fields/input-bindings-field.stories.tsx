import type { Meta, StoryObj } from '@storybook/react-vite'

import { InputBindingsField } from '#components/strategy-settings/agent-graph/fields/input-bindings-field'

const meta = {
  title: 'StrategySettings/AgentGraph/InputBindingsField',
  component: InputBindingsField,
} satisfies Meta<typeof InputBindingsField>

export default meta
type Story = StoryObj<typeof meta>

export const Configured: Story = {
  name: 'shows literal, prior output, and dataset bindings in YAML.',
  args: {
    value: {
      request: { literal: { count: 3 } },
      previous: {
        prior_output: { phase: 'prepare', pointer: '/items', limit: 25 },
      },
      source: {
        dataset: { provider: 'sample-provider', params: { code: 'DEMO' } },
      },
    },
    onChange: () => {},
  },
}

export const Empty: Story = {
  name: 'shows an empty editor for phases without input bindings.',
  args: { value: {}, onChange: () => {} },
}
