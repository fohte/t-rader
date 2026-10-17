import { Link } from '@tanstack/react-router'

type DashboardListItem = {
  key: string
  title: string
}

export type DashboardListPageState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; dashboards: DashboardListItem[] }

export function DashboardListPageView({
  state,
}: {
  state: DashboardListPageState
}) {
  return (
    <main className="space-y-5 font-sans text-foreground">
      <header className="space-y-1">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          <span className="font-mono text-primary">&gt;</span> ダッシュボード
        </h1>
        <p className="text-sm text-muted-foreground">
          保存されたダッシュボードを表示します。
        </p>
      </header>

      {state.status === 'loading' ? (
        <p
          className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground"
          role="status"
        >
          ダッシュボードを読み込んでいます。
        </p>
      ) : state.status === 'error' ? (
        <p
          className="border border-border bg-card px-4 py-8 text-center text-sm text-destructive"
          role="alert"
        >
          {state.message}
        </p>
      ) : state.dashboards.length === 0 ? (
        <p
          className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground"
          role="status"
        >
          ダッシュボードはまだありません。
        </p>
      ) : (
        <ul className="grid grid-cols-1 gap-3 md:grid-cols-2">
          {state.dashboards.map((dashboard) => (
            <li key={dashboard.key}>
              <Link
                className="block border border-border bg-card px-4 py-4 transition-colors hover:border-muted-foreground hover:bg-surface-strong focus-visible:outline-2 focus-visible:outline-ring"
                params={{ key: dashboard.key }}
                to="/dashboards/$key"
              >
                <span className="block font-semibold">{dashboard.title}</span>
                <span className="mt-1 block font-mono text-2xs text-muted-foreground">
                  {dashboard.key}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </main>
  )
}
