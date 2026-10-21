import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StrategySwitcher } from '#components/strategy-shell/strategy-switcher'
import { createStoryRouter } from '#storybook/story-router'
import { StrategySwitcherQueryDecorator } from '#storybook/strategy-switcher-mock'

function createStrategySwitcherRouter(mobileOpen: boolean) {
  return createStoryRouter(() => <StrategySwitcher mobileOpen={mobileOpen} />, {
    paths: ['/strategies/$id/performance'],
    initialPath: '/strategies/sample-alpha/performance',
  })
}

const meta = {
  title: 'StrategyShell/StrategySwitcher',
  component: StrategySwitcher,
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <StrategySwitcherQueryDecorator>
        <Story />
      </StrategySwitcherQueryDecorator>
    ),
  ],
} satisfies Meta<typeof StrategySwitcher>

export default meta
type Story = StoryObj<NonNullable<Parameters<typeof StrategySwitcher>[0]>>

export const MobileMenuOpen: Story = {
  name: 'shows the strategy links in the open mobile switcher.',
  globals: { viewport: { value: 'strategySwitcherMobile' } },
  parameters: {
    viewport: {
      options: {
        strategySwitcherMobile: {
          name: 'Strategy switcher mobile',
          styles: { width: '390px', height: '700px' },
          type: 'mobile',
        },
      },
    },
    screenshot: {
      viewport: { width: 390, height: 700 },
    },
  },
  render: () => <RouterProvider router={createStrategySwitcherRouter(true)} />,
}
