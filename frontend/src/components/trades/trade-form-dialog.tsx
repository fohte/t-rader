import { useEffect, useRef, useState } from 'react'

import { formatYen } from '#components/trades/format'
import { useInvalidateTrades } from '#components/trades/use-invalidate-trades'
import { Button } from '#components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '#components/ui/dialog'
import { Input } from '#components/ui/input'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Trade = components['schemas']['Trade']
type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']

type FormState = {
  strategyId: string
  symbol: string
  side: 'buy' | 'sell'
  qty: string
  price: string
  fee: string
  date: string
  source: 'manual' | 'csv' | 'api'
  note: string
}

const todayFormatter = new Intl.DateTimeFormat('ja-JP', {
  timeZone: 'Asia/Tokyo',
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
})

function todayJst(): string {
  // <input type="date"> 用に JST 当日を YYYY-MM-DD で取得する
  return todayFormatter.format(new Date()).replace(/\//g, '-')
}

function emptyForm(strategies: Strategy[], stocks: Stock[]): FormState {
  return {
    strategyId: strategies[0]?.id ?? '',
    symbol: stocks[0]?.id ?? '',
    side: 'buy',
    qty: '',
    price: '',
    fee: '',
    date: todayJst(),
    source: 'manual',
    note: '',
  }
}

function fromTrade(t: Trade): FormState {
  return {
    strategyId: t.strategy_id,
    symbol: t.symbol,
    side: t.side === 'sell' ? 'sell' : 'buy',
    qty: String(t.qty),
    price: String(t.price),
    fee: t.fee === 0 ? '' : String(t.fee),
    date: t.date,
    source:
      t.source === 'csv' || t.source === 'api' || t.source === 'manual'
        ? t.source
        : 'manual',
    note: t.note ?? '',
  }
}

export function TradeFormDialog({
  open,
  onOpenChange,
  initial,
  strategies,
  stocks,
  defaultStrategyId,
}: {
  open: boolean
  onOpenChange: (v: boolean) => void
  initial: Trade | null
  strategies: Strategy[]
  stocks: Stock[]
  defaultStrategyId?: string
}) {
  const invalidateTrades = useInvalidateTrades()
  const [form, setForm] = useState<FormState>(() =>
    emptyForm(strategies, stocks),
  )
  const [error, setError] = useState<string | null>(null)

  // strategies/stocks/defaultStrategyId は ref 経由で読む。
  // 開いた瞬間以外で再 reset が走ると、遅延到着で入力中の値が失われるため。
  const strategiesRef = useRef(strategies)
  strategiesRef.current = strategies
  const stocksRef = useRef(stocks)
  stocksRef.current = stocks
  const defaultStrategyIdRef = useRef(defaultStrategyId)
  defaultStrategyIdRef.current = defaultStrategyId

  useEffect(() => {
    if (!open) return
    setError(null)
    if (initial != null) {
      setForm(fromTrade(initial))
    } else {
      const base = emptyForm(strategiesRef.current, stocksRef.current)
      setForm({
        ...base,
        strategyId: defaultStrategyIdRef.current ?? base.strategyId,
      })
    }
  }, [open, initial])

  // 新規作成時にマスタデータが遅延ロードされた場合、未選択の項目だけ初期値を埋める。
  // 入力途中の値は上書きしない (未選択 = 空文字 のときだけ反応する)。
  useEffect(() => {
    if (!open || initial != null) return
    setForm((prev) => {
      const firstStrategy = strategies[0]
      const firstStock = stocks[0]
      const next = { ...prev }
      if (prev.strategyId === '' && firstStrategy != null) {
        next.strategyId = defaultStrategyId ?? firstStrategy.id
      }
      if (prev.symbol === '' && firstStock != null) {
        next.symbol = firstStock.id
      }
      return next.strategyId === prev.strategyId && next.symbol === prev.symbol
        ? prev
        : next
    })
  }, [open, initial, strategies, stocks, defaultStrategyId])

  const createMutation = $api.useMutation('post', '/api/trades', {
    onSuccess: () => {
      invalidateTrades()
      onOpenChange(false)
    },
    onError: () => {
      setError('取引の作成に失敗しました')
    },
  })
  const updateMutation = $api.useMutation('patch', '/api/trades/{id}', {
    onSuccess: () => {
      invalidateTrades()
      onOpenChange(false)
    },
    onError: () => {
      setError('取引の更新に失敗しました')
    },
  })

  const qtyNum = Number(form.qty)
  const priceNum = Number(form.price)
  const feeNum = form.fee === '' ? 0 : Number(form.fee)
  const feeValid = form.fee === '' || (Number.isFinite(feeNum) && feeNum >= 0)
  const valid =
    form.strategyId !== '' &&
    form.symbol !== '' &&
    form.qty !== '' &&
    qtyNum > 0 &&
    form.price !== '' &&
    priceNum > 0 &&
    feeValid &&
    form.date !== ''

  const set = <K extends keyof FormState>(k: K, v: FormState[K]) => {
    setForm((p) => ({ ...p, [k]: v }))
  }

  const submitting = createMutation.isPending || updateMutation.isPending

  function handleSubmit(e: React.SyntheticEvent) {
    e.preventDefault()
    if (!valid || submitting) return
    setError(null)
    const noteVal = form.note.trim() === '' ? null : form.note.trim()
    if (initial == null) {
      createMutation.mutate({
        body: {
          strategy_id: form.strategyId,
          symbol: form.symbol,
          side: form.side,
          qty: qtyNum,
          price: priceNum,
          fee: feeNum,
          date: form.date,
          source: form.source,
          note: noteVal,
        },
      })
    } else {
      updateMutation.mutate({
        params: { path: { id: initial.id } },
        body: {
          symbol: form.symbol,
          side: form.side,
          qty: qtyNum,
          price: priceNum,
          fee: feeNum,
          date: form.date,
          source: form.source,
          note: noteVal,
        },
      })
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (submitting) return
        onOpenChange(v)
      }}
    >
      <DialogContent className="sm:max-w-xl">
        <form onSubmit={handleSubmit} className="space-y-4">
          <DialogHeader>
            <DialogTitle>
              {initial != null ? '取引を編集' : '取引を追加'}
            </DialogTitle>
            <DialogDescription>
              戦略に紐づく約定を 1 件記録します。
            </DialogDescription>
          </DialogHeader>

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field label="銘柄 *">
              <SelectNative
                value={form.symbol}
                onChange={(v) => {
                  set('symbol', v)
                }}
                options={stocks.map((s) => ({
                  value: s.id,
                  label: `${s.name} (${s.id})`,
                }))}
              />
            </Field>
            <Field label="戦略 *">
              <SelectNative
                value={form.strategyId}
                onChange={(v) => {
                  set('strategyId', v)
                }}
                options={strategies.map((s) => ({
                  value: s.id,
                  label: s.name,
                }))}
                disabled={initial != null}
              />
            </Field>

            <Field label="売買 *">
              <div className="flex gap-1">
                <SideButton
                  side="buy"
                  active={form.side === 'buy'}
                  onClick={() => {
                    set('side', 'buy')
                  }}
                />
                <SideButton
                  side="sell"
                  active={form.side === 'sell'}
                  onClick={() => {
                    set('side', 'sell')
                  }}
                />
              </div>
            </Field>
            <Field label="約定日 *">
              <Input
                type="date"
                value={form.date}
                onChange={(e) => {
                  set('date', e.target.value)
                }}
              />
            </Field>

            <Field label="数量 (株) *">
              <Input
                type="number"
                inputMode="numeric"
                min={0}
                step="any"
                placeholder="100"
                value={form.qty}
                onChange={(e) => {
                  set('qty', e.target.value)
                }}
              />
            </Field>
            <Field label="単価 (¥) *">
              <Input
                type="number"
                inputMode="decimal"
                min={0}
                step="any"
                placeholder="1500"
                value={form.price}
                onChange={(e) => {
                  set('price', e.target.value)
                }}
              />
            </Field>

            <Field label="手数料 (¥)">
              <Input
                type="number"
                inputMode="numeric"
                min={0}
                step="any"
                placeholder="0"
                value={form.fee}
                onChange={(e) => {
                  set('fee', e.target.value)
                }}
              />
            </Field>
            <Field label="入力ソース">
              <SelectNative
                value={form.source}
                onChange={(v) => {
                  if (v === 'manual' || v === 'csv' || v === 'api') {
                    set('source', v)
                  }
                }}
                options={[
                  { value: 'manual', label: '手入力' },
                  { value: 'csv', label: 'CSV 取込' },
                  { value: 'api', label: 'API' },
                ]}
              />
            </Field>

            <Field label="メモ" className="sm:col-span-2">
              <Input
                type="text"
                placeholder="エントリー根拠など"
                value={form.note}
                onChange={(e) => {
                  set('note', e.target.value)
                }}
              />
            </Field>
          </div>

          {valid && (
            <div className="font-mono text-xs text-muted-foreground-strong">
              約定代金{' '}
              <span className="text-foreground">
                {formatYen(qtyNum * priceNum)}
              </span>
            </div>
          )}

          {error != null && <p className="text-xs text-primary">{error}</p>}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => {
                onOpenChange(false)
              }}
            >
              キャンセル
            </Button>
            <Button type="submit" disabled={!valid || submitting}>
              {submitting ? '送信中…' : initial != null ? '保存' : '追加'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

function Field({
  label,
  children,
  className,
}: {
  label: string
  children: React.ReactNode
  className?: string
}) {
  return (
    <label className={`flex flex-col gap-1 ${className ?? ''}`}>
      <span className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
        {label}
      </span>
      {children}
    </label>
  )
}

function SelectNative({
  value,
  onChange,
  options,
  disabled,
}: {
  value: string
  onChange: (v: string) => void
  options: { value: string; label: string }[]
  disabled?: boolean
}) {
  return (
    <select
      value={value}
      disabled={disabled}
      onChange={(e) => {
        onChange(e.target.value)
      }}
      className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50"
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  )
}

function SideButton({
  side,
  active,
  onClick,
}: {
  side: 'buy' | 'sell'
  active: boolean
  onClick: () => void
}) {
  const baseColor =
    side === 'buy' ? 'border-up text-up' : 'border-down text-down'
  const activeBg =
    side === 'buy'
      ? 'bg-[color:var(--color-up-dim)]'
      : 'bg-[color:var(--color-down-dim)]'
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex-1 border px-3 py-1.5 font-mono text-xs ${baseColor} ${
        active ? activeBg : 'bg-transparent opacity-60 hover:opacity-100'
      }`}
    >
      {side === 'buy' ? '買い' : '売り'}
    </button>
  )
}
