import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { NoteHypothesesPanelView } from '#components/note-detail/note-hypotheses-panel'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteHypothesis = components['schemas']['NoteHypothesis']
type Hypothesis = components['schemas']['Hypothesis']

const linkedHypotheses: NoteHypothesis[] = [
  {
    note_id: '00000000-0000-0000-0000-000000000010',
    hypothesis_id: '00000000-0000-0000-0000-000000000001',
    hypothesis_title: '設備投資が供給制約を緩和する',
    hypothesis_body: '複数四半期にわたり供給量と受注残を確認する。',
    hypothesis_status: 'supported',
    created_at: '2026-06-01T00:00:00Z',
  },
  {
    note_id: '00000000-0000-0000-0000-000000000010',
    hypothesis_id: '00000000-0000-0000-0000-000000000002',
    hypothesis_title: '原材料費の低下が利益率を改善する',
    hypothesis_body: '仕入価格と製品価格の推移から影響を確かめる。',
    hypothesis_status: 'unverified',
    created_at: '2026-06-02T00:00:00Z',
  },
]

const candidate: Hypothesis = {
  hypothesis_id: '00000000-0000-0000-0000-000000000003',
  strategy_id: '00000000-0000-0000-0000-000000000020',
  title: '販売経路の拡大が受注を押し上げる',
  body: '新しい販売経路の稼働後に受注の変化を比較する。',
  status: 'unverified',
  related_interest_ids: [],
  related_note_ids: [],
  created_at: '2026-06-03T00:00:00Z',
  updated_at: '2026-06-03T00:00:00Z',
}

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/hypotheses/$hypothesisId'],
        },
      )}
    />
  )
}

const meta = {
  title: 'NoteDetail/NoteHypothesesPanel',
  component: NoteHypothesesPanelView,
  parameters: { layout: 'padded' },
  args: {
    hypotheses: [],
    candidates: [],
    isPending: false,
    isError: false,
    isCandidatePending: false,
    isCandidateError: false,
    isDialogOpen: false,
    isAttaching: false,
    query: '',
    strategyAvailable: true,
    hasMutationError: false,
    hasAttachError: false,
    onDialogOpenChange: () => {},
    onQueryChange: () => {},
    onAttach: () => {},
    onRemove: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-md bg-background p-5 text-foreground">
        {withRouter(Story)}
      </div>
    ),
  ],
} satisfies Meta<typeof NoteHypothesesPanelView>

export default meta
type Story = StoryObj<typeof meta>

export const Linked: Story = {
  name: 'shows hypotheses already linked to the note.',
  args: { hypotheses: linkedHypotheses },
}

export const Empty: Story = {
  name: 'shows the panel when no hypotheses are linked to the note.',
  args: {},
}

export const NoStrategy: Story = {
  name: 'shows the message when no strategy is available.',
  args: { strategyAvailable: false },
}

export const AttachDialog: Story = {
  name: 'shows the dialog for linking an existing hypothesis to the note.',
  args: {
    candidates: [candidate],
    isDialogOpen: true,
  },
}

export const SearchMatchesCandidate: Story = {
  name: 'shows a candidate that matches the hypothesis search.',
  args: {
    candidates: [candidate],
    isDialogOpen: true,
    query: '稼働',
  },
}

export const NoMatchingCandidates: Story = {
  name: 'shows the empty search result when no hypotheses match.',
  args: {
    candidates: [],
    isDialogOpen: true,
    query: '該当なし',
  },
}
