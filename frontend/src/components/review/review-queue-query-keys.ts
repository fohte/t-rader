import { $api } from '#lib/api/client'

export function getReviewQueueQueryKeys() {
  return [
    $api.queryOptions('get', '/api/note-versions/pending').queryKey,
    $api.queryOptions('get', '/api/annotations', {
      params: { query: { status: 'unread' } },
    }).queryKey,
    $api.queryOptions('get', '/api/notes/overview').queryKey,
  ] as const
}
