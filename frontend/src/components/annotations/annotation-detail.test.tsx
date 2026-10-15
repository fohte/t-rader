import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AnnotationDetail } from '#components/annotations/annotation-detail'
import type { components } from '#lib/api/schema.gen'
import type { ExecutionOrigin } from '#lib/execution-origin'
import { createStoryRouter } from '#storybook/story-router'

type Annotation = components['schemas']['Annotation']

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
) {
  const router = createStoryRouter(
    () => (
      <AnnotationDetail
        annotation={currentAnnotation}
        isPending={false}
        reviewNavigationState={{
          navigation: null,
          isPending: false,
          isError: false,
        }}
        comments={[]}
        history={[]}
        executionOrigin={executionOrigin}
        stockName={null}
        commentDraft=""
        rejectionReason=""
        isApprovalPending={false}
        isRejectionPending={false}
        isCommentPending={false}
        approvalError={null}
        rejectionError={null}
        commentError={null}
        onApprove={() => {}}
        onReject={() => {}}
        onRejectionReasonChange={() => {}}
        onAddComment={(event) => {
          event.preventDefault()
        }}
        onCommentDraftChange={() => {}}
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
    const user = userEvent.setup()
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
    await user.type(input, 'abc')

    expect(onRejectionReasonChange.mock.calls.map(([value]) => value)).toEqual([
      'a',
      'ab',
      'abc',
    ])
  })

  it('shows the strategy and run that generated the annotation', async () => {
    renderAnnotationDetail(
      {
        ...annotation,
        execution_step_id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      },
      {
        strategyId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
        strategyName: '架空戦略',
        taskId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
      },
    )

    const strategyLink = await screen.findByRole('link', { name: '架空戦略' })
    expect(strategyLink.parentElement?.textContent).toEqual(
      '架空戦略 · 実行詳細 →',
    )
  })

  it('does not show a generation origin for an annotation written by a user', async () => {
    renderAnnotationDetail(
      { ...annotation, created_by_kind: 'human' },
      {
        strategyId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
        strategyName: '架空戦略',
        taskId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
      },
    )

    await screen.findByText(annotation.text)
    expect(
      screen.queryAllByRole('link', {
        name: /^(架空戦略|実行詳細 →)$/,
      }),
    ).toEqual([])
  })
})
