import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useEffect, useRef, useState } from 'react'

import { Button } from '#components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '#components/ui/select'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

export const Route = createFileRoute('/settings/jquants-plan')({
  component: JQuantsPlanSettingsPage,
})

type JQuantsPlan = components['schemas']['JQuantsPlan']

// 「自動 (未設定)」を表す sentinel。API 上は plan: null に対応するが、Select は
// null/undefined を値として扱えないため、選択肢用にのみ文字列を割り当てる
const AUTO_VALUE = '__auto__'

const PLAN_OPTIONS: {
  value: JQuantsPlan | typeof AUTO_VALUE
  label: string
}[] = [
  { value: AUTO_VALUE, label: '自動 (システムが検出して設定)' },
  { value: 'free', label: 'フリープラン' },
  { value: 'light', label: 'ライトプラン' },
  { value: 'standard', label: 'スタンダードプラン' },
  { value: 'premium', label: 'プレミアムプラン' },
]

function formatEffectiveRange(
  range: { from: string; to: string } | null | undefined,
) {
  if (range == null) return '検出待ち'
  return `${range.from} 〜 ${range.to}`
}

function toSelectValue(plan: JQuantsPlan | null | undefined) {
  return plan ?? AUTO_VALUE
}

const JQUANTS_PLAN_SET: Record<JQuantsPlan, true> = {
  free: true,
  light: true,
  standard: true,
  premium: true,
}

function isJQuantsPlan(value: string): value is JQuantsPlan {
  return value in JQUANTS_PLAN_SET
}

function fromSelectValue(value: string): JQuantsPlan | null {
  if (value === AUTO_VALUE) return null
  return isJQuantsPlan(value) ? value : null
}

function JQuantsPlanSettingsPage() {
  const queryClient = useQueryClient()
  const { data, isPending } = $api.useQuery('get', '/api/jquants/plan-setting')
  const mutation = $api.useMutation('put', '/api/jquants/plan-setting')

  const initialValue = toSelectValue(data?.plan)
  const [value, setValue] = useState(initialValue)
  // 前回 GET から作った initialValue。これと value が一致していれば「ユーザー未編集」と判定できる
  const lastInitialValueRef = useRef(initialValue)
  const [saveError, setSaveError] = useState<string | null>(null)

  useEffect(() => {
    if (value === lastInitialValueRef.current) {
      setValue(initialValue)
    }
    lastInitialValueRef.current = initialValue
  }, [initialValue, value])

  function handleSave() {
    setSaveError(null)
    mutation.mutate(
      { body: { plan: fromSelectValue(value) } },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/jquants/plan-setting')
              .queryKey,
          })
        },
        onError: () => {
          setSaveError('保存に失敗しました')
        },
      },
    )
  }

  const currentLabel =
    PLAN_OPTIONS.find((option) => option.value === toSelectValue(data?.plan))
      ?.label ?? '自動 (システムが検出して設定)'

  return (
    <div className="space-y-5">
      <div>
        <Link
          to="/settings"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; 設定に戻る
        </Link>
      </div>
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          設定 — J-Quants プラン
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          契約プランを手動で指定すると、そのプランの配信遅延・データ提供期間に固定して株価を取得する。「自動」は未設定のまま放置するという意味ではなく、契約範囲外エラーのメッセージから実際の取得可能範囲を検出した時点で、システムがプランを推定して一度だけ設定する。以降は継続的に追従し直すことはなく、設定を変えるまで固定される。
        </p>
      </header>

      {isPending ? (
        <Skeleton className="h-40 w-full max-w-sm" />
      ) : (
        <div className="max-w-sm space-y-4">
          <p
            data-testid="current-value"
            className="font-mono text-xs text-muted-foreground-strong"
          >
            現在の設定: {currentLabel}
            <br />
            現在の取得範囲: {formatEffectiveRange(data?.effective_range)}
          </p>
          <div className="space-y-1.5">
            <label
              htmlFor="jquants-plan"
              className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
            >
              契約プラン
            </label>
            <Select
              items={PLAN_OPTIONS}
              value={value}
              onValueChange={(next) => {
                if (next != null) setValue(next)
              }}
            >
              <SelectTrigger
                id="jquants-plan"
                aria-label="契約プラン"
                className="w-full"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PLAN_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <Button
              type="button"
              onClick={handleSave}
              disabled={mutation.isPending}
            >
              {mutation.isPending ? '保存中…' : '保存'}
            </Button>
            {saveError != null && (
              <span
                data-testid="save-error"
                className="font-mono text-xs text-primary"
              >
                {saveError}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
