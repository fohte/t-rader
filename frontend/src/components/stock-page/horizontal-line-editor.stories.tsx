import type { Meta, StoryObj } from '@storybook/react-vite'

import { HorizontalLineEditor } from '#components/stock-page/horizontal-line-editor'

const meta = {
  title: 'StockPage/HorizontalLineEditor',
  component: HorizontalLineEditor,
  args: {
    timestamp: '2030-04-02T06:00:00Z',
    price: 3110,
    targetKinds: ['sample-kind', 'sample-kind-2'],
    onSave: () => undefined,
    onCancel: () => undefined,
  },
} satisfies Meta<typeof HorizontalLineEditor>

export default meta
type Story = StoryObj<typeof meta>

export const NewLine: Story = {
  name: 'shows the note form for a newly placed horizontal line.',
}

export const Saving: Story = {
  name: 'shows the form while the line is being saved.',
  args: { isSaving: true },
}

export const SaveFailed: Story = {
  name: 'shows an error when the line could not be saved.',
  args: { errorMessage: '保存に失敗しました' },
}
