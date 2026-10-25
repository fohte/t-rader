import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/note-versions/pending')({
  beforeLoad: () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error, no-restricted-syntax -- TanStack Router の redirect API は throw を要求する
    throw redirect({ to: '/notes', search: { view: 'pending' } })
  },
})
