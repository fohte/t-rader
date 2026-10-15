import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { NoteEvidenceCardView } from '#components/note-detail/note-evidence-card-view'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'NoteDetail/NoteEvidenceCardView',
  component: NoteEvidenceCardView,
  args: {
    open: true,
    onOpenChange: () => undefined,
    displayValue: '3,150',
    reference: {
      kind: 'price',
      stockName: 'デモ商事',
      stockId: '0000',
      field: 'close',
      date: '2030-01-03',
    },
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/stocks/$stockId', '/indicators/$indicatorId'],
          },
        )}
      />
    ),
  ],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof NoteEvidenceCardView>

export default meta
type Story = StoryObj<typeof meta>

export const Price: Story = {
  name: 'shows the date and source for a price reference.',
}

export const Change: Story = {
  name: 'shows the date range and calculation for a change reference.',
  args: {
    displayValue: '+8.62%',
    reference: {
      kind: 'change',
      stockName: 'デモ商事',
      stockId: '0000',
      field: 'close',
      start: '2030-01-02',
      end: '2030-01-03',
      barsState: {
        status: 'ready',
        evidence: {
          startValue: 2900,
          endValue: 3150,
          changePercent: (250 / 2900) * 100,
          matchesSavedValue: true,
        },
      },
    },
  },
}

export const ChangeWithDifferentSavedValue: Story = {
  name: 'explains when the saved value differs from current daily bars.',
  args: {
    displayValue: '+7.5%',
    reference: {
      kind: 'change',
      stockName: 'デモ商事',
      stockId: '0000',
      field: 'close',
      start: '2030-01-02',
      end: '2030-01-03',
      barsState: {
        status: 'ready',
        evidence: {
          startValue: 2900,
          endValue: 3150,
          changePercent: (250 / 2900) * 100,
          matchesSavedValue: false,
        },
      },
    },
  },
}

export const LoadingBars: Story = {
  name: 'shows a loading state while daily bars are requested.',
  args: {
    displayValue: '+8.62%',
    reference: {
      kind: 'change',
      stockName: 'デモ商事',
      stockId: '0000',
      field: 'close',
      start: '2030-01-02',
      end: '2030-01-03',
      barsState: { status: 'loading' },
    },
  },
}

export const FailedBars: Story = {
  name: 'shows an error when daily bars cannot be retrieved.',
  args: {
    displayValue: '+8.62%',
    reference: {
      kind: 'change',
      stockName: 'デモ商事',
      stockId: '0000',
      field: 'close',
      start: '2030-01-02',
      end: '2030-01-03',
      barsState: { status: 'error' },
    },
  },
}

export const MissingEndpointBars: Story = {
  name: 'shows a message when either date has no daily bar.',
  args: {
    displayValue: '+8.62%',
    reference: {
      kind: 'change',
      stockName: 'デモ商事',
      stockId: '0000',
      field: 'close',
      start: '2030-01-02',
      end: '2030-01-03',
      barsState: { status: 'empty' },
    },
  },
}

export const Valuation: Story = {
  name: 'shows an unadjusted valuation and its stock page destination.',
  args: {
    displayValue: '18.4',
    reference: {
      kind: 'valuation',
      stockName: 'デモ商事',
      stockId: '0000',
      field: 'per',
      date: '2030-01-03',
    },
  },
}

export const SplitAdjustedValuation: Story = {
  name: 'shows when an earnings valuation is adjusted for stock splits.',
  args: {
    displayValue: '125',
    reference: {
      kind: 'valuation',
      stockName: 'デモ商事',
      stockId: '0000',
      field: 'eps',
      date: '2030-01-03',
    },
  },
}

export const Indicator: Story = {
  name: 'shows a dated macro indicator with its resolved name.',
  args: {
    displayValue: '142.5',
    reference: {
      kind: 'indicator',
      indicatorName: 'デモ指標',
      indicatorId: 'demo-indicator',
      date: '2030-01-03',
    },
  },
}

export const IndicatorNameUnavailable: Story = {
  name: 'falls back to the indicator ID when its name is unavailable.',
  args: {
    displayValue: '142.5',
    reference: {
      kind: 'indicator',
      indicatorName: null,
      indicatorId: 'demo-indicator',
      date: '2030-01-03',
    },
  },
}
