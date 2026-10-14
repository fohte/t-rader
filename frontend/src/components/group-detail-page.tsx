import { ReferenceDetailState } from '#components/reference-detail-state'
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
      isHoldingsLoading: boolean
      isHoldingsError: boolean
      notes: Note[] | null
      isNotesError: boolean
    }

export function GroupDetailPage(props: GroupDetailPageProps) {
  if (props.state !== 'loaded') {
    return (
      <ReferenceDetailState
        state={props.state}
        label="グループ"
        identifier={`${props.axisKey}/${props.groupKey}`}
      />
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
          {props.group.stocks.length > 0 && props.isHoldingsLoading && (
            <p
              className="font-mono text-2xs text-muted-foreground"
              role="status"
            >
              保有状況を読み込んでいます。
            </p>
          )}
          {props.group.stocks.length > 0 && props.isHoldingsError && (
            <p className="font-mono text-2xs text-destructive" role="status">
              保有状況を読み込めませんでした。
            </p>
          )}
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
                  </tr>
                </thead>
                <tbody>
                  {props.group.stocks.map((stock) => (
                    <tr
                      key={stock.id}
                      className="border-b border-border last:border-b-0 hover:bg-surface-strong"
                    >
                      <td className="px-3 py-2">
                        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                          <StockNameLink
                            name={stock.name}
                            symbol={stock.id}
                            missingName="link-symbol"
                          />
                          {!props.isHoldingsLoading &&
                            !props.isHoldingsError &&
                            heldStockIds.has(stock.id) && (
                              <span className="border border-primary/50 px-1.5 py-0.5 font-mono text-2xs text-primary">
                                保有
                              </span>
                            )}
                        </div>
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
