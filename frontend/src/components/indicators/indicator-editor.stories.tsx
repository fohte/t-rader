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
  name: 'rsi',
  code: `import json, sys

args = json.load(sys.stdin)["args"]
period = args.get("period", 14)
print(json.dumps({"value": period * 2}))
`,
  inputSchema: JSON.stringify(
    {
      type: 'object',
      properties: { period: { type: 'integer' } },
      required: ['period'],
    },
    null,
    2,
  ),
  outputSchema: JSON.stringify(
    {
      type: 'object',
      properties: { value: { type: 'number' } },
      required: ['value'],
    },
    null,
    2,
  ),
  description: 'RSI 風サンプル',
}

const QUIET: PreviewState = { isRunning: false, error: null, result: null }

export const GlobalEdit: Story = {
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
  args: {
    scope: 'strategy',
    initial: { ...INITIAL, name: '', description: '' },
    onSave: () => {},
    onPreview: () => {},
    preview: QUIET,
  },
}

export const PreviewSuccess: Story = {
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
        output: { value: 28 },
        stdout: '{"value": 28}\n',
        stderr: '',
        exit_code: 0,
      },
    },
  },
}

export const PreviewSandboxRejected: Story = {
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
