import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider, useRouterState } from '@tanstack/react-router'

import {
  parseStrategySettingsTab,
  StrategySettingsTabs,
} from '#components/strategy-settings/settings-tabs'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'StrategySettings/SettingsTabs',
  parameters: { layout: 'padded' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

function renderTabs(initialTab: 'general' | 'triggers' | 'indicators') {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <StorySettingsTabs />
        ),
        {
          paths: ['/strategies/$id/settings'],
          initialPath: `/strategies/demo-strategy/settings?settingsTab=${initialTab}`,
        },
      )}
    />
  )
}

function StorySettingsTabs() {
  const activeTab = useRouterState({
    select: (state) =>
      parseStrategySettingsTab(state.location.search.settingsTab) ?? 'general',
  })
  return (
    <StrategySettingsTabs strategyId="demo-strategy" activeTab={activeTab} />
  )
}

export const GeneralSelected: Story = {
  name: 'the general tab is selected.',
  render: () => renderTabs('general'),
}

export const TriggersSelected: Story = {
  name: 'the triggers tab is selected.',
  render: () => renderTabs('triggers'),
}

export const IndicatorsSelected: Story = {
  name: 'the custom indicators tab is selected.',
  render: () => renderTabs('indicators'),
}
