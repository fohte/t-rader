import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { AnnotationStockReference } from '#components/annotations/annotation-stock-reference'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Annotations/AnnotationStockReference',
  component: AnnotationStockReference,
  parameters: { layout: 'padded' },
  args: { symbol: 'demo-code' },
  render: (args) => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <div className="font-mono text-xs text-muted-foreground-strong">
            <AnnotationStockReference {...args} />
          </div>
        ),
        { paths: ['/charts/$instrumentId'] },
      )}
    />
  ),
} satisfies Meta<typeof AnnotationStockReference>

export default meta
type Story = StoryObj<typeof meta>

export const Resolved: Story = {
  name: '解決済みの銘柄名を日足チャートへのリンクとして表示します。',
  args: { name: '架空商事', symbol: 'demo-code' },
}

export const Unresolved: Story = {
  name: '未解決の銘柄名はチャートリンクにせずコードを表示します。',
  args: { name: null, symbol: 'demo-code' },
}
