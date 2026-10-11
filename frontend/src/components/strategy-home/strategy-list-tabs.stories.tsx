import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import {
  type StrategyListTab,
  StrategyListTabs,
} from '#components/strategy-home/strategy-list-tabs'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'StrategyHome/StrategyListTabs',
  parameters: { layout: 'padded' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

function renderTabs(activeTab: StrategyListTab) {
  const router = createStoryRouter(
    () => <StrategyListTabs activeTab={activeTab} />,
    {
      paths: ['/strategies'],
      initialPath: `/strategies?strategyTab=${activeTab}`,
    },
  )
  return <RouterProvider router={router} />
}

export const ListSelected: Story = {
  name: 'the strategy list tab is selected.',
  render: () => renderTabs('list'),
}

export const RunsSelected: Story = {
  name: 'the run history tab is selected.',
  render: () => renderTabs('runs'),
}

export const IndicatorsSelected: Story = {
  name: 'the indicators tab is selected.',
  render: () => renderTabs('indicators'),
}

export const Mobile: Story = {
  name: 'the tabs fit on a 375px screen.',
  render: () => (
    <div className="bg-background px-3 pt-5" style={{ width: 375 }}>
      {renderTabs('runs')}
    </div>
  ),
}
