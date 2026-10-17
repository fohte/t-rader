import { WidgetToplist, WidgetTreemap } from '#components/widgets'
import type { components } from '#lib/api/schema.gen'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetVisualization = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' | 'toplist' }
>

export type MarkdownBodyWidgetState =
  | { kind: 'loading'; asOf: string }
  | {
      kind: 'definition-error'
      asOf?: string
      message: string
    }
  | {
      kind: 'data-error'
      asOf?: string
      message: string
    }
  | {
      kind: 'ready'
      result: WidgetQueryResponse
      viz: WidgetVisualization
    }

export function MarkdownBodyWidgetView({
  state,
}: {
  state: MarkdownBodyWidgetState
}) {
  const asOf = state.kind === 'ready' ? state.result.as_of : state.asOf

  return (
    <section
      aria-label="ノート内ウィジェット"
      className="border border-border bg-background"
    >
      <header className="flex items-center justify-between gap-3 border-b border-border px-3 py-2">
        <h4 className="font-mono text-2xs font-semibold uppercase tracking-wider text-muted-foreground-strong">
          ウィジェット
        </h4>
        {asOf != null && (
          <time
            className="font-mono text-2xs tabular-nums text-muted-foreground"
            dateTime={asOf}
          >
            基準日 {asOf}
          </time>
        )}
      </header>
      <div className="p-3">
        {state.kind === 'loading' ? (
          <p
            className="py-6 text-center font-mono text-xs text-muted-foreground"
            role="status"
          >
            ウィジェットを読み込み中…
          </p>
        ) : state.kind === 'definition-error' ? (
          <p
            className="py-4 text-center font-mono text-xs text-primary"
            role="alert"
          >
            ウィジェットの定義を表示できません: {state.message}
          </p>
        ) : state.kind === 'data-error' ? (
          <p
            className="py-4 text-center font-mono text-xs text-primary"
            role="alert"
          >
            データを取得できませんでした: {state.message}
          </p>
        ) : state.viz.type === 'treemap' ? (
          <WidgetTreemap result={state.result} viz={state.viz} />
        ) : (
          <WidgetToplist result={state.result} viz={state.viz} />
        )}
      </div>
    </section>
  )
}
