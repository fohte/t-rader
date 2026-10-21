import type { Meta, StoryObj } from '@storybook/react-vite'

import type {
  IndicatorEditorValue,
  PreviewState,
} from '#components/indicators/indicator-editor'
import { IndicatorEditorView } from '#components/indicators/indicator-editor-view'

const value: IndicatorEditorValue = {
  name: 'demo-indicator',
  code: `def evaluate(args):
    return {"result": args.get("value", 0)}
`,
  inputSchema: JSON.stringify(
    {
      type: 'object',
      properties: { value: { type: 'number' } },
    },
    null,
    2,
  ),
  outputSchema: JSON.stringify(
    {
      type: 'object',
      properties: { result: { type: 'number' } },
    },
    null,
    2,
  ),
  description: '表示確認用の架空指標です。',
}

const quietPreview: PreviewState = {
  isRunning: false,
  error: null,
  result: null,
}

const meta = {
  title: 'Indicators/IndicatorEditorView',
  component: IndicatorEditorView,
  args: {
    scope: 'global',
    value,
    isDirty: false,
    nameReadOnly: true,
    onPatch: () => {},
    argsJson: '{"value": 7}',
    onArgsJsonChange: () => {},
    onSave: () => {},
    isSaving: false,
    saveError: null,
    onPreview: () => {},
    preview: quietPreview,
  },
} satisfies Meta<typeof IndicatorEditorView>

export default meta
type Story = StoryObj<typeof meta>

export const Dirty: Story = {
  name: 'the editor marks an unsaved change and enables saving.',
  args: {
    value: { ...value, description: '編集した架空の説明です。' },
    isDirty: true,
    nameReadOnly: false,
  },
}

export const Saving: Story = {
  name: 'the editor disables saving while a change is being saved.',
  args: { isDirty: true, isSaving: true },
}

export const SaveError: Story = {
  name: 'the editor shows a save error beside the save button.',
  args: { isDirty: true, saveError: '保存に失敗しました。' },
}

export const PreviewRunning: Story = {
  name: 'the editor disables preview while it is running.',
  args: { preview: { ...quietPreview, isRunning: true } },
}

export const PreviewError: Story = {
  name: 'the editor shows a validation error from the preview form.',
  args: {
    preview: {
      isRunning: false,
      error: 'args の形式を確認してください。',
      result: null,
    },
  },
}
