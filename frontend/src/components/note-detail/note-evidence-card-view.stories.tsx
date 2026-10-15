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
    stockName: 'デモ商事',
    stockId: '0000',
    field: 'close',
    displayValue: '3,150',
    evidence: { kind: 'price', date: '2030-01-03' },
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/stocks/$stockId'],
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
  args: { evidence: { kind: 'price', date: '2030-01-03' } },
}

export const Change: Story = {
  name: 'shows the date range and calculation for a change reference.',
  args: {
    displayValue: '+8.62%',
    evidence: {
      kind: 'change',
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
    evidence: {
      kind: 'change',
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
    evidence: {
      kind: 'change',
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
    evidence: {
      kind: 'change',
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
    evidence: {
      kind: 'change',
      start: '2030-01-02',
      end: '2030-01-03',
      barsState: { status: 'empty' },
    },
  },
}
