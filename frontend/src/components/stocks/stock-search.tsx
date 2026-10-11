import { Input } from '@fohte/ui/input'
import { Link } from '@tanstack/react-router'
import { Search } from 'lucide-react'
import { useState } from 'react'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Stock = components['schemas']['Stock']

export function StockSearchView({
  open,
  onOpenChange,
  query,
  onQueryChange,
  stocks,
  isPending,
  isError,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  query: string
  onQueryChange: (query: string) => void
  stocks: Stock[]
  isPending: boolean
  isError: boolean
}) {
  const trimmedQuery = query.trim()

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger
        render={
          <button
            type="button"
            aria-label="銘柄を探す"
            title="銘柄を探す"
            className="inline-flex h-7 w-7 flex-shrink-0 items-center justify-center gap-1.5 border border-border px-0 py-1 font-mono text-xs text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground md:h-auto md:w-auto md:px-2.5"
          >
            <Search aria-hidden="true" className="size-3.5" />
            <span className="hidden md:inline">銘柄を探す</span>
          </button>
        }
      />
      <PopoverContent
        align="end"
        sideOffset={8}
        className="w-80 space-y-3 p-3"
        style={{ maxWidth: 'calc(100vw - 1.5rem)' }}
      >
        <div className="space-y-2">
          <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-muted-foreground-strong">
            <span className="mr-1 text-primary">&gt;</span>
            銘柄を探す
          </h2>
          <Input
            aria-label="銘柄名またはコードで検索"
            placeholder="銘柄名またはコードで検索"
            value={query}
            onChange={(event) => {
              onQueryChange(event.currentTarget.value)
            }}
          />
        </div>

        <div aria-live="polite" className="max-h-72 overflow-y-auto">
          {trimmedQuery === '' ? (
            <p className="px-2 py-3 text-sm text-muted-foreground">
              銘柄名またはコードを入力してください。
            </p>
          ) : isPending ? (
            <p className="px-2 py-3 text-sm text-muted-foreground">検索中...</p>
          ) : isError ? (
            <p className="px-2 py-3 text-sm text-primary">
              検索に失敗しました。
            </p>
          ) : stocks.length === 0 ? (
            <p className="px-2 py-3 text-sm text-muted-foreground">
              該当する銘柄はありません。
            </p>
          ) : (
            <ul className="divide-y divide-border border-y border-border">
              {stocks.map((stock) => (
                <li key={stock.id}>
                  <Link
                    to="/charts/$instrumentId"
                    params={{ instrumentId: stock.id }}
                    onClick={() => {
                      onOpenChange(false)
                    }}
                    className="flex items-center justify-between gap-3 px-2 py-2 text-sm text-foreground hover:bg-surface-strong"
                  >
                    <span className="min-w-0 truncate">{stock.name}</span>
                    <span className="shrink-0 font-mono text-xs text-muted-foreground">
                      {stock.id}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}

export function StockSearch() {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const normalizedQuery = query.trim()
  const stocksQuery = $api.useQuery(
    'get',
    '/api/refs/stocks',
    { params: { query: { q: normalizedQuery } } },
    { enabled: open && normalizedQuery !== '' },
  )

  return (
    <StockSearchView
      open={open}
      onOpenChange={setOpen}
      query={query}
      onQueryChange={setQuery}
      stocks={stocksQuery.data ?? []}
      isPending={stocksQuery.isPending}
      isError={stocksQuery.isError}
    />
  )
}
