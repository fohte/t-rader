import { Link } from '@tanstack/react-router'

import type { DashboardConfigIssue } from '#components/dashboard-pages/dashboard-config'
import { WidgetToplist, WidgetTreemap } from '#components/widgets'
import type { components } from '#lib/api/schema.gen'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

export type DashboardWidgetPageState = {
  key: string
  title: string
  width: 'half' | 'full'
  definition: {
    viz: Extract<
      components['schemas']['WidgetVisualizationRequest'],
      { type: 'treemap' | 'toplist' }
    >
  }
  isPending: boolean
  errorMessage: string | null
  result: WidgetQueryResponse | null
}

export type DashboardPageState =
  | { status: 'loading'; title: string }
  | { status: 'error'; title: string; message: string }
  | { status: 'not-found'; title: string }
  | {
      status: 'invalid'
      title: string
      issues: DashboardConfigIssue[]
    }
  | {
      status: 'ready'
      title: string
      asOf: string
      onAsOfChange: (date: string) => void
      widgets: DashboardWidgetPageState[]
    }

function PageHeading({
  title,
  showDashboardListLink,
}: {
  title: string
  showDashboardListLink: boolean
}) {
  return (
    <header className="flex flex-wrap items-end justify-between gap-3">
      <div className="space-y-2">
        {showDashboardListLink ? (
          <Link
            className="inline-flex items-center font-mono text-2xs text-muted-foreground hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"
            to="/dashboards"
          >
            ← ダッシュボード一覧
          </Link>
        ) : null}
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          <span className="font-mono text-primary">&gt;</span> {title}
        </h1>
      </div>
    </header>
  )
}

function StateMessage({
  title,
  message,
  role,
  showDashboardListLink,
}: {
  title: string
  message: string
  role: 'status' | 'alert'
  showDashboardListLink: boolean
}) {
  return (
    <main className="space-y-5 font-sans text-foreground">
      <PageHeading
        title={title}
        showDashboardListLink={showDashboardListLink}
      />
      <p
        className="border border-border bg-card px-4 py-8 text-center text-sm text-muted-foreground"
        role={role}
      >
        {message}
      </p>
    </main>
  )
}

function WidgetCard({ widget }: { widget: DashboardWidgetPageState }) {
  return (
    <article
      className={`min-w-0 space-y-3 border border-border bg-card p-3 ${widget.width === 'full' ? 'md:col-span-2' : ''}`}
    >
      <h2 className="font-mono text-xs font-semibold text-foreground">
        {widget.title}
      </h2>
      {widget.isPending ? (
        <p
          className="border border-border px-4 py-8 text-center font-mono text-xs text-muted-foreground"
          role="status"
        >
          ウィジェットを読み込んでいます。
        </p>
      ) : widget.errorMessage != null ? (
        <p
          className="border border-border px-4 py-8 text-center text-xs text-destructive"
          role="alert"
        >
          {widget.errorMessage}
        </p>
      ) : widget.result == null ? (
        <p
          className="border border-border px-4 py-8 text-center text-xs text-destructive"
          role="alert"
        >
          ウィジェットのデータを取得できませんでした。
        </p>
      ) : widget.definition.viz.type === 'treemap' ? (
        <WidgetTreemap result={widget.result} viz={widget.definition.viz} />
      ) : (
        <WidgetToplist result={widget.result} viz={widget.definition.viz} />
      )}
    </article>
  )
}

export function DashboardPageView({
  state,
  showDashboardListLink = true,
}: {
  state: DashboardPageState
  showDashboardListLink?: boolean
}) {
  if (state.status === 'loading') {
    return (
      <StateMessage
        title={state.title}
        message="ダッシュボードを読み込んでいます。"
        role="status"
        showDashboardListLink={showDashboardListLink}
      />
    )
  }
  if (state.status === 'error') {
    return (
      <StateMessage
        title={state.title}
        message={state.message}
        role="alert"
        showDashboardListLink={showDashboardListLink}
      />
    )
  }
  if (state.status === 'not-found') {
    return (
      <StateMessage
        title={state.title}
        message="指定したダッシュボードは見つかりませんでした。"
        role="alert"
        showDashboardListLink={showDashboardListLink}
      />
    )
  }
  if (state.status === 'invalid') {
    return (
      <main className="space-y-5 font-sans text-foreground">
        <PageHeading
          title={state.title}
          showDashboardListLink={showDashboardListLink}
        />
        <section
          aria-labelledby="dashboard-config-errors-title"
          className="border border-destructive/50 bg-card p-4"
        >
          <h2
            className="font-mono text-xs font-semibold text-destructive"
            id="dashboard-config-errors-title"
          >
            ダッシュボード設定に問題があります
          </h2>
          <ul
            className="mt-3 space-y-2 text-xs text-muted-foreground"
            role="alert"
          >
            {state.issues.map((issue, index) => (
              <li key={`${issue.path}-${String(index)}`}>
                <span className="font-mono text-foreground">{issue.path}</span>
                <span>: {issue.message}</span>
              </li>
            ))}
          </ul>
        </section>
      </main>
    )
  }

  return (
    <main className="space-y-5 font-sans text-foreground">
      <PageHeading
        title={state.title}
        showDashboardListLink={showDashboardListLink}
      />
      <div className="flex flex-wrap items-center justify-between gap-3 border-y border-border py-3">
        <p className="font-mono text-xs text-muted-foreground">
          基準日の初期値は最新です。日付を選ぶと全ウィジェットに適用します。
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-2 font-mono text-xs text-muted-foreground-strong">
            基準日
            <input
              aria-label="基準日"
              className="border border-border bg-background px-2 py-1 text-foreground focus-visible:outline-2 focus-visible:outline-ring"
              onChange={(event) => {
                state.onAsOfChange(event.currentTarget.value)
              }}
              type="date"
              value={state.asOf}
            />
          </label>
          <button
            aria-pressed={state.asOf === ''}
            className="border border-border px-2 py-1 font-mono text-xs text-muted-foreground-strong hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring aria-pressed:bg-muted aria-pressed:text-foreground"
            onClick={() => {
              state.onAsOfChange('')
            }}
            type="button"
          >
            最新
          </button>
        </div>
      </div>

      {state.widgets.length === 0 ? (
        <p
          className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground"
          role="status"
        >
          このダッシュボードにはウィジェットがありません。
        </p>
      ) : (
        <section
          aria-label="ダッシュボードのウィジェット"
          className="grid grid-cols-1 gap-3 md:grid-cols-2"
        >
          {state.widgets.map((widget) => (
            <WidgetCard key={widget.key} widget={widget} />
          ))}
        </section>
      )}
    </main>
  )
}
