import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'

import { Button } from '#components/ui/button'
import { Input } from '#components/ui/input'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import { formatRatioPercent, parseRatioPercent } from '#lib/ratio-percent'

interface RiskPolicyTabProps {
  strategyId: string
}

export function RiskPolicyTab({ strategyId }: RiskPolicyTabProps) {
  const queryClient = useQueryClient()
  const { data, isPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}/risk-policy',
    { params: { path: { id: strategyId } } },
  )
  const mutation = $api.useMutation('put', '/api/strategies/{id}/risk-policy')

  const initialInput = formatRatioPercent(data?.max_position_ratio)
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

  if (isPending) {
    return <Skeleton className="h-40 w-full" />
  }

  function handleSave() {
    const parsed = parseRatioPercent(input)
    if (parsed.error != null) {
      setValidationError(parsed.error)
      return
    }
    setValidationError(null)
    setSaveError(null)
    mutation.mutate(
      {
        params: { path: { id: strategyId } },
        body: { max_position_ratio: parsed.ratio },
      },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions(
              'get',
              '/api/strategies/{id}/risk-policy',
              { params: { path: { id: strategyId } } },
            ).queryKey,
          })
        },
        onError: () => {
          setSaveError('保存に失敗しました')
        },
      },
    )
  }

  return (
    <div className="max-w-sm space-y-4">
      <p
        data-testid="current-value"
        className="font-mono text-xs text-muted-foreground-strong"
      >
        現在の上限:{' '}
        {data?.max_position_ratio != null
          ? `${formatRatioPercent(data.max_position_ratio)}%`
          : '上限なし'}
      </p>
      <div className="space-y-1.5">
        <label
          htmlFor="max-position-ratio"
          className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          1 銘柄あたりの保有時価上限 (投資可能額に対する割合)
        </label>
        <div className="flex items-center gap-2">
          <Input
            id="max-position-ratio"
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
  )
}
