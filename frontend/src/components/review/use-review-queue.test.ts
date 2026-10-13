import { describe, expect, it } from 'vitest'

import { getReviewQueueQueryKeys } from '#components/review/review-queue-query-keys'
import { $api } from '#lib/api/client'

describe('getReviewQueueQueryKeys', () => {
  it('returns the pending note-version and unread annotation query keys', () => {
    expect(getReviewQueueQueryKeys()).toEqual([
      $api.queryOptions('get', '/api/note-versions/pending').queryKey,
      $api.queryOptions('get', '/api/annotations', {
        params: { query: { status: 'unread' } },
      }).queryKey,
    ])
  })
})
