import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  AgentConfigPurposeView,
  type AgentConfigTab,
} from '#components/strategy-settings/agent-configs-purpose-view'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'StrategySettings/AgentConfigs/PurposeView',
  component: AgentConfigPurposeView,
} satisfies Meta<typeof AgentConfigPurposeView>

export default meta
type Story = StoryObj<typeof meta>

function renderPurposeView(
  state: 'loading' | 'not-found' | 'ready',
  tab: AgentConfigTab = 'agents-md',
) {
  function PurposeView() {
    if (state !== 'ready') {
      return <AgentConfigPurposeView state={state} />
    }

    return (
      <AgentConfigPurposeView
        state="ready"
        purpose="sample-purpose"
        tab={tab}
        onTabChange={() => {}}
      >
        <div className="h-48 border border-border" />
      </AgentConfigPurposeView>
    )
  }

  const router = createStoryRouter(() => <PurposeView />, {
    paths: ['/settings/agent-configs', '/settings/agent-configs/$purpose'],
    initialPath: '/settings/agent-configs/sample-purpose',
  })
  return <RouterProvider router={router} />
}

export const Loading: Story = {
  name: 'shows a loading placeholder while a purpose is fetched.',
  args: { state: 'loading' },
  render: () => renderPurposeView('loading'),
}

export const NotFound: Story = {
  name: 'shows a message when the requested purpose is missing.',
  args: { state: 'not-found' },
  render: () => renderPurposeView('not-found'),
}

export const AgentGraphSelected: Story = {
  name: 'shows the agent graph tab selected for a purpose.',
  args: {
    state: 'ready',
    purpose: 'sample-purpose',
    tab: 'agent-graph',
    onTabChange: () => {},
    children: <div className="h-48 border border-border" />,
  },
  render: () => renderPurposeView('ready', 'agent-graph'),
}
