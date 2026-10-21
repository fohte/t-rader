import type { Meta, StoryObj } from '@storybook/react-vite'

import { AgentGraphViewSelector } from '#components/strategy-settings/agent-graph/view-selector'

const meta = {
  title: 'StrategySettings/AgentGraph/ViewSelector',
  component: AgentGraphViewSelector,
} satisfies Meta<typeof AgentGraphViewSelector>

export default meta
type Story = StoryObj<typeof meta>

const onChange = () => {}

export const FormSelected: Story = {
  name: 'shows the form view selected when the graph can be edited.',
  args: { value: 'form', formAvailable: true, onChange },
}

export const YamlSelected: Story = {
  name: 'shows the YAML view selected when the graph can be edited.',
  args: { value: 'yaml', formAvailable: true, onChange },
}

export const FormUnavailable: Story = {
  name: 'shows only YAML when the graph cannot be parsed as a form.',
  args: { value: 'yaml', formAvailable: false, onChange },
}
