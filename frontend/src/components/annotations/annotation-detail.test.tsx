import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AnnotationDetail } from '#components/annotations/annotation-detail'
import type { components } from '#lib/api/schema.gen'
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
      reviewNavigation={null}
      reviewNavigationPending={false}
      reviewNavigationError={false}
      comments={[]}
      history={[]}
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
        paths: ['/annotations/$annoId', '/notes', '/stocks/$stockId'],
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
})
