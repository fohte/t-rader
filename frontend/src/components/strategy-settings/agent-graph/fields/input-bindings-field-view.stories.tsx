import type { Meta, StoryObj } from '@storybook/react-vite'

import { InputBindingsFieldView } from '#components/strategy-settings/agent-graph/fields/input-bindings-field-view'

const meta = {
  title: 'StrategySettings/AgentGraph/InputBindingsFieldView',
  component: InputBindingsFieldView,
} satisfies Meta<typeof InputBindingsFieldView>

export default meta
type Story = StoryObj<typeof meta>

export const Configured: Story = {
  name: 'shows valid input bindings in the YAML editor.',
  args: {
    value: 'request:\n  literal:\n    count: 3\n',
    invalidInput: false,
    onChange: () => {},
  },
}

export const InvalidInput: Story = {
  name: 'shows a validation hint when the YAML is not a mapping.',
  args: {
    value: 'request: [',
    invalidInput: true,
    onChange: () => {},
  },
}
