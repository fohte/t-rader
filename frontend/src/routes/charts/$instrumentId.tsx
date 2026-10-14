import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/charts/$instrumentId')({
  beforeLoad: ({ params }) => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error, no-restricted-syntax -- TanStack Router の redirect API は throw を要求する
    throw redirect({
      to: '/stocks/$stockId/chart',
      params: { stockId: params.instrumentId },
    })
  },
})
