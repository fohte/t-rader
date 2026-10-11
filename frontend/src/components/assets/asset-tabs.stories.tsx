import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { AssetTabs } from '#components/assets/asset-tabs'
import { createStoryRouter } from '#storybook/story-router'

function createAssetTabsRouter(
  activeTab: Parameters<typeof AssetTabs>[0]['activeTab'],
) {
  return createStoryRouter(() => <AssetTabs activeTab={activeTab} />, {
    paths: ['/portfolio', '/trades'],
    initialPath: '/portfolio',
  })
}

const meta = {
  title: 'Assets/AssetTabs',
  component: AssetTabs,
  render: ({ activeTab }) => (
    <RouterProvider router={createAssetTabsRouter(activeTab)} />
  ),
} satisfies Meta<typeof AssetTabs>

export default meta
type Story = StoryObj<typeof meta>

export const Holdings: Story = {
  name: 'shows holdings as the selected asset tab.',
  args: { activeTab: 'holdings' },
}

export const Trades: Story = {
  name: 'shows trades as the selected asset tab.',
  args: { activeTab: 'trades' },
}

export const NarrowViewport: Story = {
  name: 'fits both asset tabs within a 375 pixel wide container.',
  args: { activeTab: 'holdings' },
  decorators: [
    (Story) => (
      <div className="border border-border p-3" style={{ width: 375 }}>
        <Story />
      </div>
    ),
  ],
}
