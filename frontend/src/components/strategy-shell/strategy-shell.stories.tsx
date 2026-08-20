import type { Meta, StoryObj } from '@storybook/react-vite'
import { Outlet, RouterProvider } from '@tanstack/react-router'

import { StrategyShell } from '#components/strategy-shell/strategy-shell'
import { createStoryRouter } from '#storybook/story-router'
import { StrategySwitcherQueryDecorator } from '#storybook/strategy-switcher-mock'

const PLACEHOLDER_PATHS = [
  { path: '/strategies', label: '戦略一覧 placeholder' },
  { path: '/strategies/$id', label: '戦略ホーム placeholder' },
  { path: '/portfolio', label: 'ポートフォリオ placeholder' },
  { path: '/trades', label: '取引履歴 placeholder' },
].map(({ path, label }) => ({
  path,
  component: () => (
    <div className="font-mono text-sm text-[color:var(--color-text-secondary)]">
      {label}
    </div>
  ),
}))

function createStrategyShellRouter(initialPath: string) {
  return createStoryRouter(
    () => (
      <StrategyShell>
        <Outlet />
      </StrategyShell>
    ),
    { paths: PLACEHOLDER_PATHS, initialPath },
  )
}

const meta = {
  title: 'StrategyShell/StrategyShell',
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <StrategySwitcherQueryDecorator>
        <Story />
      </StrategySwitcherQueryDecorator>
    ),
  ],
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => (
    <RouterProvider router={createStrategyShellRouter('/strategies')} />
  ),
}

export const StrategyHome: Story = {
  render: () => (
    <RouterProvider
      router={createStrategyShellRouter('/strategies/semi-swing')}
    />
  ),
}
