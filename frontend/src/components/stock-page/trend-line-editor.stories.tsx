import type { Meta, StoryObj } from '@storybook/react-vite'

import { TrendLineEditor } from '#components/stock-page/trend-line-editor'

const meta = {
  title: 'StockPage/TrendLineEditor',
  component: TrendLineEditor,
  args: {
    start: { timestamp: '2030-04-01T06:00:00Z', price: 3390 },
    end: { timestamp: '2030-04-02T06:00:00Z', price: 3120 },
    targetKinds: ['sample-kind', 'sample-kind-2'],
    onSave: () => undefined,
    onCancel: () => undefined,
  },
} satisfies Meta<typeof TrendLineEditor>

export default meta
type Story = StoryObj<typeof meta>

export const NewLine: Story = {
  name: 'shows the note form for a newly placed trendline.',
}

export const Saving: Story = {
  name: 'shows the form while the trendline is being saved.',
  args: { isSaving: true },
}

export const SaveFailed: Story = {
  name: 'shows an error when the trendline could not be saved.',
  args: { errorMessage: '保存に失敗しました' },
}
