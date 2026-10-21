import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  IndicatorEditor,
  type IndicatorEditorValue,
  type PreviewState,
} from '#components/indicators/indicator-editor'

const meta = {
  title: 'Indicators/IndicatorEditor',
  component: IndicatorEditor,
} satisfies Meta<typeof IndicatorEditor>

export default meta
type Story = StoryObj<typeof meta>

const INITIAL: IndicatorEditorValue = {
  name: 'demo-indicator',
  code: `import json, sys

args = json.load(sys.stdin)["args"]
value = args.get("value", 0)
print(json.dumps({"result": value}))
`,
  inputSchema: JSON.stringify(
    {
      type: 'object',
      properties: { value: { type: 'number' } },
      required: ['value'],
    },
    null,
    2,
  ),
  outputSchema: JSON.stringify(
    {
      type: 'object',
      properties: { result: { type: 'number' } },
      required: ['result'],
    },
    null,
    2,
  ),
  description: '表示確認用の架空指標です。',
}

const QUIET: PreviewState = { isRunning: false, error: null, result: null }

export const GlobalEdit: Story = {
  name: 'shows the editor for a global indicator with a fixed name.',
  args: {
    scope: 'global',
    initial: INITIAL,
    nameReadOnly: true,
    onSave: () => {},
    onPreview: () => {},
    preview: QUIET,
  },
}

export const StrategyCreate: Story = {
  name: 'shows a blank form for creating a strategy-specific indicator.',
  args: {
    scope: 'strategy',
    initial: { ...INITIAL, name: '', description: '' },
    onSave: () => {},
    onPreview: () => {},
    preview: QUIET,
  },
}

export const PreviewSuccess: Story = {
  name: 'shows a successful indicator preview with its output value.',
  args: {
    scope: 'global',
    initial: INITIAL,
    nameReadOnly: true,
    onSave: () => {},
    onPreview: () => {},
    preview: {
      isRunning: false,
      error: null,
      result: {
        output: { result: 7 },
        stdout: '{"result": 7}\n',
        stderr: '',
        exit_code: 0,
      },
    },
  },
}

export const PreviewSandboxRejected: Story = {
  name: 'shows a sandbox error when the preview script requests network access.',
  args: {
    scope: 'global',
    initial: INITIAL,
    nameReadOnly: true,
    onSave: () => {},
    onPreview: () => {},
    preview: {
      isRunning: false,
      error: null,
      result: {
        output: null,
        stdout: '',
        stderr: 'PermissionError: network access denied',
        exit_code: 1,
      },
    },
  },
}

export const Saving: Story = {
  name: 'shows the global indicator editor while changes are saving.',
  args: {
    scope: 'global',
    initial: INITIAL,
    nameReadOnly: true,
    isSaving: true,
    onSave: () => {},
    onPreview: () => {},
    preview: QUIET,
  },
}
