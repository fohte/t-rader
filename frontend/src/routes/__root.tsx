import { createRootRoute, Outlet } from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'
import { ErrorBoundary } from 'react-error-boundary'

import { AppShell } from '@/components/app-shell'
import { ErrorFallback } from '@/components/error-fallback'

export const Route = createRootRoute({
  component: RootComponent,
  notFoundComponent: () => (
    <div className="flex h-full items-center justify-center">
      <div className="text-center">
        <h1 className="text-4xl font-bold">404</h1>
        <p className="mt-2 text-muted-foreground">
          ページが見つかりませんでした
        </p>
      </div>
    </div>
  ),
})

function RootComponent() {
  return (
    <ErrorBoundary FallbackComponent={ErrorFallback}>
      <AppShell>
        <Outlet />
      </AppShell>
      <TanStackRouterDevtools position="bottom-right" />
    </ErrorBoundary>
  )
}
