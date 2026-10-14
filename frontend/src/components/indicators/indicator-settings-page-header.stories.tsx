import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { IndicatorSettingsPageHeader } from '#components/indicators/indicator-settings-page-header'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Indicators/IndicatorSettingsPageHeader',
  parameters: { layout: 'padded' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

function renderHeader(scope: 'global' | 'strategy') {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <IndicatorSettingsPageHeader scope={scope} />
        ),
        {
          paths: [
            '/settings',
            '/settings/indicators',
            '/strategies/$id/settings',
          ],
          initialPath:
            scope === 'global'
              ? '/settings/indicators'
              : '/strategies/demo-strategy/settings?settingsTab=indicators',
        },
      )}
    />
  )
}

export const StrategyScope: Story = {
  name: 'the strategy scope links to shared custom indicators.',
  render: () => renderHeader('strategy'),
}

export const GlobalScope: Story = {
  name: 'the shared scope links back to global settings.',
  render: () => renderHeader('global'),
}
