import { RelatedNotes } from '#components/related-notes'
import { StockNameLink } from '#components/stocks/stock-name-link'
import type { components } from '#lib/api/schema.gen'

type Group = components['schemas']['Group']
type Note = components['schemas']['Note']

type GroupDetailPageProps =
  | { state: 'loading'; axisKey: string; groupKey: string }
  | { state: 'not-found'; axisKey: string; groupKey: string }
  | { state: 'error'; axisKey: string; groupKey: string }
  | {
      state: 'loaded'
      group: Group
      heldStockIds: string[]
      notes: Note[] | null
      isNotesError: boolean
    }

export function GroupDetailPage(props: GroupDetailPageProps) {
  if (props.state === 'loading') {
    return (
      <main className="space-y-5 font-sans text-foreground" aria-busy="true">
        <div className="space-y-3">
          <div className="h-8 w-48 animate-pulse bg-surface-strong" />
          <div className="h-9 w-36 animate-pulse bg-surface-strong" />
        </div>
        <p className="font-mono text-xs text-muted-foreground">
          グループを読み込んでいます: {props.axisKey}/{props.groupKey}
        </p>
      </main>
    )
  }

  if (props.state === 'not-found') {
    return (
      <main className="space-y-2 font-sans text-foreground">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          グループが見つかりません
        </h1>
        <p className="font-mono text-sm text-muted-foreground">
          {props.axisKey}/{props.groupKey}
        </p>
      </main>
    )
  }

  if (props.state === 'error') {
    return (
      <main className="space-y-2 font-sans text-foreground">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          グループを読み込めませんでした
        </h1>
        <p className="font-mono text-sm text-muted-foreground">
          {props.axisKey}/{props.groupKey}
        </p>
      </main>
    )
  }

  const heldStockIds = new Set(props.heldStockIds)
  const description = props.group.description?.trim()

  return (
    <main className="space-y-5 font-sans text-foreground">
      <header className="space-y-2">
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <h1 className="text-2xl font-bold leading-tight tracking-tight">
            {props.group.name}
          </h1>
          <span className="font-mono text-xs text-muted-foreground">
            {props.group.axis.name} / {props.group.axis.key}/{props.group.key}
          </span>
        </div>
        {description != null && description.length > 0 && (
          <p className="max-w-180 text-sm leading-relaxed text-muted-foreground-strong">
            {description}
          </p>
        )}
      </header>

      <div className="grid grid-cols-1 items-start gap-5 lg:grid-cols-(--grid-cols-portfolio-layout)">
        <section className="space-y-2" aria-labelledby="group-stocks-title">
          <div className="flex items-baseline justify-between gap-3">
            <h2
              id="group-stocks-title"
              className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
            >
              <span className="mr-2 text-primary">&gt;</span>
              銘柄
            </h2>
            <span className="shrink-0 font-mono text-2xs text-muted-foreground">
              {props.group.stocks.length} 件
            </span>
          </div>
          <div className="overflow-x-auto border border-border bg-card">
            {props.group.stocks.length === 0 ? (
              <p className="px-4 py-7 text-center font-mono text-xs text-muted-foreground">
                このグループに銘柄はありません。
              </p>
            ) : (
              <table className="w-full font-mono text-xs">
                <thead>
                  <tr className="border-b border-border text-2xs uppercase tracking-wider text-muted-foreground">
                    <th className="px-3 py-2 text-left font-normal">銘柄</th>
                    <th className="px-3 py-2 text-right font-normal">
                      保有状況
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {props.group.stocks.map((stock) => (
                    <tr
                      key={stock.id}
                      className="border-b border-border last:border-b-0 hover:bg-surface-strong"
                    >
                      <td className="px-3 py-2">
                        <StockNameLink
                          name={stock.name}
                          symbol={stock.id}
                          missingName="link-symbol"
                        />
                      </td>
                      <td className="px-3 py-2 text-right">
                        {heldStockIds.has(stock.id) && (
                          <span className="border border-primary/50 px-1.5 py-0.5 font-mono text-2xs text-primary">
                            保有
                          </span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </section>

        <RelatedNotes notes={props.notes} isError={props.isNotesError} />
      </div>
    </main>
  )
}
