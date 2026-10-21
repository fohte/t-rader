import type { Meta, StoryObj } from '@storybook/react-vite'

import type {
  IndicatorEditorValue,
  PreviewState,
} from '#components/indicators/indicator-editor'
import { IndicatorsPageView } from '#components/indicators/indicators-page-view'

const initial: IndicatorEditorValue = {
  name: 'demo-indicator-a',
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
  description: '架空の値を変換する指標です。',
}

const emptyInitial: IndicatorEditorValue = {
  ...initial,
  name: '',
  code: '',
  description: '',
}

const quietPreview: PreviewState = {
  isRunning: false,
  error: null,
  result: null,
}

const meta = {
  title: 'Indicators/IndicatorsPageView',
  component: IndicatorsPageView,
  args: {
    scope: 'global',
    indicators: [
      { id: 'demo-id-a', name: 'demo-indicator-a' },
      { id: 'demo-id-b', name: 'demo-indicator-b' },
    ],
    isPending: false,
    selectedId: 'demo-id-a',
    isCreating: false,
    initial,
    isSaving: false,
    saveError: null,
    preview: quietPreview,
    onSelectIndicator: () => {},
    onSave: () => {},
    onPreview: () => {},
  },
} satisfies Meta<typeof IndicatorsPageView>

export default meta
type Story = StoryObj<typeof meta>

export const ExistingIndicator: Story = {
  name: 'the selected indicator tab shows its editor.',
}

export const CreatingIndicator: Story = {
  name: 'the create tab shows a blank indicator editor.',
  args: {
    indicators: [],
    selectedId: null,
    isCreating: true,
    initial: emptyInitial,
  },
}

export const Loading: Story = {
  name: 'the page shows a placeholder while indicators load.',
  args: {
    indicators: [],
    selectedId: null,
    isPending: true,
  },
}

export const Empty: Story = {
  name: 'the page invites the user to create the first indicator.',
  args: { indicators: [], selectedId: null },
}
