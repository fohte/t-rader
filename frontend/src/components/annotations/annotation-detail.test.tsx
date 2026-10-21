import { RouterProvider } from '@tanstack/react-router'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import {
  type ComponentProps,
  type MouseEvent,
  type SyntheticEvent,
  useState,
} from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AnnotationDetail } from '#components/annotations/annotation-detail'
import type { components } from '#lib/api/schema.gen'
import type { ExecutionOrigin } from '#lib/execution-origin'
import { createStoryRouter } from '#storybook/story-router'
import { getExecutionOriginLinks } from '#test/execution-origin-links.test-helper'

type Annotation = components['schemas']['Annotation']
type AnnotationDetailProps = ComponentProps<typeof AnnotationDetail>

const annotation: Annotation = {
  id: 'sample-annotation-01',
  target_kind: 'stock',
  target_symbol: 'sample-code',
  timestamp: '2026-10-15T01:00:00Z',
  price: null,
  text: 'サンプルのアノテーション本文です。',
  linked_note_id: null,
  status: 'unread',
  created_by_kind: 'llm',
  execution_step_id: null,
  execution_task_id: null,
  created_at: '2026-10-15T01:00:00Z',
  updated_at: '2026-10-15T01:00:00Z',
}

const executionOrigin: ExecutionOrigin = {
  strategyId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
  strategyName: '架空戦略',
  taskId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
}

afterEach(cleanup)

function StatefulAnnotationDetail({
  onRejectionReasonChange,
}: {
  onRejectionReasonChange: (value: string) => void
}) {
  const [rejectionReason, setRejectionReason] = useState('')

  return (
    <AnnotationDetail
      annotation={annotation}
      isPending={false}
      reviewNavigationState={{
        navigation: null,
        isPending: false,
        isError: false,
      }}
      comments={[]}
      history={[]}
      executionOrigin={null}
      stockName="サンプル銘柄"
      commentDraft=""
      rejectionReason={rejectionReason}
      isApprovalPending={false}
      isRejectionPending={false}
      isCommentPending={false}
      approvalError={null}
      rejectionError={null}
      commentError={null}
      onApprove={() => {}}
      onReject={() => {}}
      onRejectionReasonChange={(value) => {
        onRejectionReasonChange(value)
        setRejectionReason(value)
      }}
      onAddComment={(event) => {
        event.preventDefault()
      }}
      onCommentDraftChange={() => {}}
    />
  )
}

function renderAnnotationDetail(
  currentAnnotation: Annotation,
  executionOrigin: ExecutionOrigin | null,
  overrides: Partial<AnnotationDetailProps> = {},
) {
  const router = createStoryRouter(
    () => (
      <AnnotationDetail
        {...({
          annotation: currentAnnotation,
          isPending: false,
          reviewNavigationState: {
            navigation: null,
            isPending: false,
            isError: false,
          },
          comments: [],
          history: [],
          executionOrigin,
          stockName: null,
          commentDraft: '',
          rejectionReason: '',
          isApprovalPending: false,
          isRejectionPending: false,
          isCommentPending: false,
          approvalError: null,
          rejectionError: null,
          commentError: null,
          onApprove: () => {},
          onReject: () => {},
          onRejectionReasonChange: () => {},
          onAddComment: (event) => {
            event.preventDefault()
          },
          onCommentDraftChange: () => {},
          ...overrides,
        } satisfies AnnotationDetailProps)}
      />
    ),
    {
      paths: [
        '/annotations/$annoId',
        '/strategies/$id',
        '/strategies/$id/runs/$taskId',
      ],
      initialPath: '/annotations/sample-annotation-01',
    },
  )
  render(<RouterProvider router={router} />)
}

describe('AnnotationDetail', () => {
  it('passes the rejection reason input to its change callback', async () => {
    const onRejectionReasonChange = vi.fn<(value: string) => void>()
    const router = createStoryRouter(
      () => (
        <StatefulAnnotationDetail
          onRejectionReasonChange={onRejectionReasonChange}
        />
      ),
      {
        paths: [
          '/annotations/$annoId',
          '/notes',
          '/stocks/$stockId',
          '/strategies/$id',
          '/strategies/$id/runs/$taskId',
        ],
        initialPath: '/annotations/sample-annotation-01',
      },
    )
    render(<RouterProvider router={router} />)

    const input = await screen.findByRole('textbox', {
      name: '却下理由 (任意)',
    })
    fireEvent.change(input, { target: { value: 'abc' } })

    expect(onRejectionReasonChange.mock.calls).toEqual([['abc']])
  })

  it('sends the approval action through its callback', async () => {
    const user = userEvent.setup()
    const onApprove = vi.fn<(event?: MouseEvent<HTMLButtonElement>) => void>()
    renderAnnotationDetail(annotation, null, { onApprove })

    await user.click(await screen.findByRole('button', { name: '✓ 承認' }))

    expect(onApprove.mock.calls.map(([event]) => event?.type ?? null)).toEqual([
      'click',
    ])
  })

  it('sends the rejection action through its callback', async () => {
    const user = userEvent.setup()
    const onReject = vi.fn<(event?: MouseEvent<HTMLButtonElement>) => void>()
    renderAnnotationDetail(annotation, null, { onReject })

    await user.click(await screen.findByRole('button', { name: '✕ 却下' }))

    expect(onReject.mock.calls.map(([event]) => event?.type ?? null)).toEqual([
      'click',
    ])
  })

  it('prevents submission when sending a comment', async () => {
    const user = userEvent.setup()
    const onAddComment = vi.fn((event: SyntheticEvent<HTMLFormElement>) => {
      event.preventDefault()
    })
    renderAnnotationDetail(annotation, null, {
      commentDraft: '架空のコメント',
      onAddComment,
    })

    await user.click(await screen.findByRole('button', { name: '送信' }))

    expect(
      onAddComment.mock.calls.map(([event]) => event.defaultPrevented),
    ).toEqual([true])
  })

  it('shows the strategy and run that generated the annotation', async () => {
    renderAnnotationDetail(
      {
        ...annotation,
        execution_step_id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      },
      executionOrigin,
    )

    const strategyLink = await screen.findByRole('link', { name: '架空戦略' })
    expect(strategyLink.parentElement?.textContent).toEqual(
      '架空戦略 · 実行詳細 →',
    )
  })

  it.each([
    {
      state: 'the annotation was written by a user',
      currentAnnotation: { ...annotation, created_by_kind: 'human' },
      origin: executionOrigin,
    },
    {
      state: 'the generation origin cannot be resolved',
      currentAnnotation: annotation,
      origin: null,
    },
  ])(
    'omits origin links when $state',
    async ({ currentAnnotation, origin }) => {
      renderAnnotationDetail(currentAnnotation, origin)

      await screen.findByText(annotation.text)
      expect(getExecutionOriginLinks()).toEqual([])
    },
  )
})
