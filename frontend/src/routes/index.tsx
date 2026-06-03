import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/')({
  beforeLoad: () => {
    // TanStack Router の慣習: `throw redirect(...)` でリダイレクトを表現する
    // eslint-disable-next-line @typescript-eslint/only-throw-error
    throw redirect({ to: '/strategies' })
  },
})
