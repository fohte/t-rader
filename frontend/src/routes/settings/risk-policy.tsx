import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useEffect, useRef, useState } from 'react'

import { Button } from '#components/ui/button'
import { Input } from '#components/ui/input'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import { formatRatioPercent, parseRatioPercent } from '#lib/ratio-percent'

export const Route = createFileRoute('/settings/risk-policy')({
  component: RiskPolicySettingsPage,
})

function RiskPolicySettingsPage() {
  const queryClient = useQueryClient()
  const { data, isPending } = $api.useQuery('get', '/api/account/risk-policy')
  const mutation = $api.useMutation('put', '/api/account/risk-policy')

  const initialInput = formatRatioPercent(data?.max_sector_ratio)
  const [input, setInput] = useState(initialInput)
  // 前回 GET から作った initialInput。これと input が一致していれば「ユーザー未編集」と判定できる
  const lastInitialInputRef = useRef(initialInput)
  const [validationError, setValidationError] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)

  useEffect(() => {
    if (input === lastInitialInputRef.current) {
      setInput(initialInput)
    }
    lastInitialInputRef.current = initialInput
  }, [initialInput, input])

  function handleSave() {
    const parsed = parseRatioPercent(input)
    if (parsed.error != null) {
      setValidationError(parsed.error)
      return
    }
    setValidationError(null)
    setSaveError(null)
    mutation.mutate(
      { body: { max_sector_ratio: parsed.ratio } },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/account/risk-policy')
              .queryKey,
          })
        },
        onError: () => {
          setSaveError('保存に失敗しました')
        },
      },
    )
  }

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
          設定 — リスク上限
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          口座全体の保有銘柄時価合計に対する、1
          セクターの保有時価合計の上限比率を設定します。
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
            現在の上限:{' '}
            {data?.max_sector_ratio != null
              ? `${formatRatioPercent(data.max_sector_ratio)}%`
              : '上限なし'}
          </p>
          <div className="space-y-1.5">
            <label
              htmlFor="max-sector-ratio"
              className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
            >
              1 セクターあたりの保有時価上限 (口座全体に対する割合)
            </label>
            <div className="flex items-center gap-2">
              <Input
                id="max-sector-ratio"
                inputMode="decimal"
                value={input}
                placeholder="上限なし"
                aria-invalid={validationError != null}
                onChange={(e) => {
                  setInput(e.target.value)
                  if (validationError != null) setValidationError(null)
                }}
              />
              <span className="font-mono text-xs text-muted-foreground">%</span>
            </div>
            {validationError != null && (
              <p
                data-testid="validation-error"
                className="font-mono text-2xs text-primary"
              >
                {validationError}
              </p>
            )}
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
