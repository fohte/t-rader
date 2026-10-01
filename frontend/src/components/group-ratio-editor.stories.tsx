import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  GroupRatioEditor,
  type GroupRatioEditorProps,
} from '#components/group-ratio-editor'

const axes = [
  { key: 'example-axis-a', name: 'Example dimension A' },
  { key: 'example-axis-b', name: 'Example dimension B' },
]

const NOOP = (): void => {}
const NOOP_ROWS: GroupRatioEditorProps['onRowsChange'] = () => {}

const meta = {
  title: 'Settings/GroupRatioEditor',
  component: GroupRatioEditor,
  args: {
    axes,
    rows: [],
    errors: [],
    isLoading: false,
    loadError: false,
    isSaving: false,
    saveError: null,
    onRowsChange: NOOP_ROWS,
    onSave: NOOP,
  },
} satisfies Meta<typeof GroupRatioEditor>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: 'shows a loading placeholder while limits are fetched.',
  args: { isLoading: true },
}

export const LoadError: Story = {
  name: 'shows an error when loading limits fails.',
  args: { loadError: true },
}

export const Empty: Story = {
  name: 'shows that no group ratio limits are configured.',
}

export const Configured: Story = {
  name: 'shows limits configured for two classification axes.',
  args: {
    rows: [
      { axis: 'example-axis-a', ratioPercent: '25' },
      { axis: 'example-axis-b', ratioPercent: '40' },
    ],
  },
}

export const ValidationError: Story = {
  name: 'shows a validation error for an incomplete limit.',
  args: {
    rows: [{ axis: '', ratioPercent: '' }],
    errors: [{ axis: '分類軸を選択してください' }],
  },
}

export const Saving: Story = {
  name: 'disables editing while the limit is being saved.',
  args: {
    rows: [{ axis: 'example-axis-a', ratioPercent: '25' }],
    isSaving: true,
  },
}

export const SaveError: Story = {
  name: 'shows an error after saving the limit fails.',
  args: {
    rows: [{ axis: 'example-axis-a', ratioPercent: '25' }],
    saveError: '保存に失敗しました',
  },
}
