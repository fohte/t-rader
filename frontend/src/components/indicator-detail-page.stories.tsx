import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { IndicatorDetailPage } from '#components/indicator-detail-page'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']

const notes: Note[] = [
  {
    id: 'fake-note-a',
    version_id: 'fake-version-a',
    version_no: 1,
    is_current: true,
    title: '為替変動を確認するメモ',
    body_md: '表示用の架空ノートです。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: [],
    kind: null,
    status: 'approved',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'llm',
    created_at: '2026-10-09T03:00:00Z',
    updated_at: '2026-10-09T03:00:00Z',
  },
]

const observations = [
  { date: '2026-10-08', value: 151.52 },
  { date: '2026-10-09', value: 152.34 },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/notes', '/notes/$noteId'] },
      )}
    />
  )
}

const meta = {
  title: 'References/IndicatorDetailPage',
  component: IndicatorDetailPage,
  decorators: [withRouter],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof IndicatorDetailPage>

export default meta
type Story = StoryObj<typeof meta>

const loadedArgs = {
  state: 'loaded',
  indicator: { id: 'DEMO_FX', name: 'デモ為替', kind: 'macro' },
  observations,
  visibleObservations: observations,
  isObservationsLoading: false,
  isObservationsError: false,
  notes,
  isNotesError: false,
  period: '1y',
  onPeriodChange: () => {},
} satisfies Story['args']

export const Loaded: Story = {
  name: 'the indicator page shows its latest value and related notes.',
  args: loadedArgs,
}

export const NotFound: Story = {
  name: 'the page explains when an indicator does not exist.',
  args: { state: 'not-found', indicatorId: 'demo-missing' },
}

export const Loading: Story = {
  name: 'the page shows a placeholder while the indicator loads.',
  args: { state: 'loading', indicatorId: 'demo-indicator' },
}

export const Error: Story = {
  name: 'the page explains when the indicator cannot be loaded.',
  args: { state: 'error', indicatorId: 'demo-indicator' },
}

export const NoObservations: Story = {
  name: 'the page explains when the indicator has no observations.',
  args: {
    ...loadedArgs,
    observations: [],
    visibleObservations: [],
  },
}

export const LatestValueError: Story = {
  name: 'the page explains when the latest value cannot be loaded.',
  args: {
    ...loadedArgs,
    observations: null,
    visibleObservations: [],
    isObservationsError: true,
  },
}

export const ObservationsLoading: Story = {
  name: 'the page shows placeholders while observations load.',
  args: {
    ...loadedArgs,
    observations: null,
    visibleObservations: [],
    isObservationsLoading: true,
  },
}

export const NoPreviousValue: Story = {
  name: 'the page omits the change when there is no previous value.',
  args: {
    ...loadedArgs,
    observations: [{ date: '2026-10-09', value: 152.34 }],
    visibleObservations: [{ date: '2026-10-09', value: 152.34 }],
  },
}

export const NoRelatedNotes: Story = {
  name: 'the page explains when no notes reference the indicator.',
  args: {
    ...loadedArgs,
    notes: [],
  },
}
