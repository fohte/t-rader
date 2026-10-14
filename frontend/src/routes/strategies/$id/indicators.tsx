import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/strategies/$id/indicators')({
  beforeLoad: ({ params }) => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error, no-restricted-syntax -- TanStack Router の redirect API は throw を要求する
    throw redirect({
      to: '/strategies/$id/settings',
      params: { id: params.id },
      search: { settingsTab: 'indicators' },
    })
  },
})
