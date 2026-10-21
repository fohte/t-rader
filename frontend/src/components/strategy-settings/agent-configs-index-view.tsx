import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'
import { Input } from '@fohte/ui/input'
import { Link } from '@tanstack/react-router'

import { Skeleton } from '#components/ui/skeleton'

interface AgentConfigsIndexViewProps {
  purposes: readonly string[]
  isPending: boolean
  newPurpose: string
  isCreating: boolean
  createError: string | null
  onPurposeChange: (value: string) => void
  onCreate: () => void
  onDelete: (purpose: string) => void
}

export function AgentConfigsIndexView({
  purposes,
  isPending,
  newPurpose,
  isCreating,
  createError,
  onPurposeChange,
  onCreate,
  onDelete,
}: AgentConfigsIndexViewProps) {
  return (
    <div className="space-y-5">
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          設定 — Agent 設定
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          目的ごとに AGENTS.md / skills / agent graph
          を管理します。目的は資金の区分とは独立しています。
        </p>
      </header>

      <div className="max-w-sm space-y-1.5">
        <label htmlFor="new-agent-config-purpose">
          <Chip>新しい目的</Chip>
        </label>
        <div className="flex gap-2">
          <Input
            id="new-agent-config-purpose"
            value={newPurpose}
            placeholder="例: sample-purpose"
            aria-invalid={createError != null}
            onChange={(event) => {
              onPurposeChange(event.target.value)
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault()
                onCreate()
              }
            }}
          />
          <Button type="button" onClick={onCreate} disabled={isCreating}>
            追加
          </Button>
        </div>
        {createError != null && (
          <p
            data-testid="create-error"
            className="font-mono text-2xs text-primary"
          >
            {createError}
          </p>
        )}
      </div>

      {isPending ? (
        <Skeleton className="h-40 w-full" />
      ) : purposes.length === 0 ? (
        <div className="border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
          まだ Agent
          設定が登録されていません。上のフォームから追加してください。
        </div>
      ) : (
        <ul data-testid="agent-config-list" className="border border-border">
          {purposes.map((purpose) => (
            <li
              key={purpose}
              className="flex items-center justify-between border-b border-border px-1 last:border-b-0"
            >
              <Link
                to="/settings/agent-configs/$purpose"
                params={{ purpose }}
                className="flex-1 truncate px-3 py-2"
              >
                <span className="font-mono text-sm">{purpose}</span>
              </Link>
              <div className="font-mono text-2xs">
                <Button
                  type="button"
                  variant="plain"
                  onClick={() => {
                    onDelete(purpose)
                  }}
                  aria-label={`"${purpose}" を削除`}
                >
                  削除
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
