import type { Meta, StoryObj } from '@storybook/react-vite'

import { PredictionsPanelView } from '#components/note-detail/predictions-panel'
import type { components } from '#lib/api/schema.gen'

type Prediction = components['schemas']['Prediction']

const basePrediction: Prediction = {
  prediction_id: '00000000-0000-0000-0000-000000000001',
  strategy_id: 'semi-swing',
  note_id: '00000000-0000-0000-0000-000000000010',
  target_stock_id: '3436',
  benchmark_stock_id: '9999.T',
  direction: 'outperform',
  probability: 0.72,
  base_date: '2026-06-01',
  due_date: '2026-06-30',
  created_at: '2026-06-01T00:00:00Z',
}

const meta = {
  title: 'NoteDetail/PredictionsPanel',
  component: PredictionsPanelView,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="max-w-md bg-background p-5 text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PredictionsPanelView>

export default meta
type Story = StoryObj<typeof meta>

export const Outperform: Story = {
  args: {
    predictions: [basePrediction],
    stockNameById: new Map([
      ['3436', 'SUMCO'],
      ['9999.T', 'TOPIX'],
    ]),
  },
}

export const Underperform: Story = {
  args: {
    predictions: [{ ...basePrediction, direction: 'underperform' }],
    stockNameById: new Map([
      ['3436', 'SUMCO'],
      ['9999.T', 'TOPIX'],
    ]),
  },
}

export const UnresolvedStockName: Story = {
  args: {
    predictions: [basePrediction],
    stockNameById: new Map(),
  },
}

export const MultiplePredictions: Story = {
  args: {
    predictions: [
      basePrediction,
      {
        ...basePrediction,
        prediction_id: '00000000-0000-0000-0000-000000000002',
        target_stock_id: '6920',
        direction: 'underperform',
        probability: 0.55,
      },
    ],
    stockNameById: new Map([
      ['3436', 'SUMCO'],
      ['6920', 'レーザーテック'],
      ['9999.T', 'TOPIX'],
    ]),
  },
}
