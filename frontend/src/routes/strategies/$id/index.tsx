import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/strategies/$id/')({
  beforeLoad: ({ params }) => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error, no-restricted-syntax -- TanStack Router redirect API requires throw
    throw redirect({ to: '/strategies/$id/performance', params })
  },
})
