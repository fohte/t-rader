import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StockNameLink } from '#components/stocks/stock-name-link'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Stocks/StockNameLink',
  component: StockNameLink,
  parameters: { layout: 'padded' },
  render: (args) => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <div className="flex items-baseline gap-2 font-mono text-xs text-muted-foreground-strong">
            <StockNameLink {...args} />
          </div>
        ),
        { paths: ['/charts/$instrumentId'] },
      )}
    />
  ),
} satisfies Meta<typeof StockNameLink>

export default meta
type Story = StoryObj<typeof meta>

export const ResolvedName: Story = {
  name: '解決済みの銘柄名とコードを表示し、名前だけをリンクにします。',
  args: {
    name: '架空商事',
    symbol: 'demo-code',
    missingName: 'link-symbol',
  },
}

export const MissingNameUsesSymbol: Story = {
  name: '銘柄名が未登録の場合はコードをリンクとして表示します。',
  args: { name: null, symbol: 'demo-code', missingName: 'link-symbol' },
}

export const UnresolvedName: Story = {
  name: '銘柄名が未解決の場合はコードだけを表示します。',
  args: { name: null, symbol: 'demo-code', missingName: 'unresolved' },
}
