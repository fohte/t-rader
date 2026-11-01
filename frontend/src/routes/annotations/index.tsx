import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/annotations/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { view?: 'all' | 'pending'; kind?: string; tag?: string } => ({
    view:
      search.view === 'all' || search.view === 'pending'
        ? search.view
        : undefined,
    kind:
      typeof search.kind === 'string' && search.kind.length > 0
        ? search.kind
        : undefined,
    tag:
      typeof search.tag === 'string' && search.tag.length > 0
        ? search.tag
        : undefined,
  }),
  beforeLoad: ({ search }) => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error, no-restricted-syntax -- TanStack Router の redirect API は throw を要求する
    throw redirect({
      to: '/notes',
      search: {
        tab: 'annotations',
        view: search.view,
        kind: search.kind,
        tag: search.tag,
      },
    })
  },
})
