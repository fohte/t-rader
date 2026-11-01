import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'
import { useState } from 'react'

export interface TaskRunDefinitionSnapshot {
  definitionKey: string
  taskId: string
  digest: string
  createdAt: string
  definition: unknown
}

export interface TaskRunDefinitionSnapshotPanelProps {
  snapshots: TaskRunDefinitionSnapshot[]
  state: 'loading' | 'error' | 'ready'
  initialSelectedKey?: string
  definitionBodyOpen?: boolean
}

export function TaskRunDefinitionSnapshotPanel({
  snapshots,
  state,
  initialSelectedKey,
  definitionBodyOpen = false,
}: TaskRunDefinitionSnapshotPanelProps) {
  const [selectedKey, setSelectedKey] = useState<string | null>(
    initialSelectedKey ?? null,
  )
  const selectedSnapshot =
    snapshots.find((snapshot) => snapshot.definitionKey === selectedKey) ??
    snapshots.find((snapshot) => snapshot.definitionKey === 'task_start') ??
    snapshots[0] ??
    null

  return (
    <section
      aria-label="実行時の固定定義"
      className="border border-border bg-card p-4"
    >
      <h2 className="mb-2 font-mono text-xs font-bold uppercase tracking-wider">
        実行時の固定定義
      </h2>
      {state === 'loading' ? (
        <p className="font-mono text-xs text-muted-foreground">
          固定定義を読み込み中です。
        </p>
      ) : state === 'error' ? (
        <p className="font-mono text-xs text-destructive">
          固定定義を取得できませんでした。
        </p>
      ) : selectedSnapshot == null ? (
        <p className="font-mono text-xs text-muted-foreground">
          固定定義の記録がありません。
        </p>
      ) : (
        <>
          <Select
            value={selectedSnapshot.definitionKey}
            onValueChange={setSelectedKey}
          >
            <SelectTrigger aria-label="固定定義 snapshot">
              <SelectValue>{selectedSnapshot.definitionKey}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {snapshots.map((snapshot) => (
                <SelectItem
                  key={snapshot.definitionKey}
                  value={snapshot.definitionKey}
                >
                  {snapshot.definitionKey}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <dl
            data-testid="definition-snapshot-metadata"
            className="mt-2 grid grid-cols-(--grid-cols-step-detail) gap-x-2.5 gap-y-1 font-mono text-2xs text-muted-foreground"
          >
            <dt>定義キー</dt>
            <dd className="break-all text-foreground">
              {selectedSnapshot.definitionKey}
            </dd>
            <dt>SHA-256</dt>
            <dd className="break-all text-foreground">
              {selectedSnapshot.digest}
            </dd>
            <dt>記録日時</dt>
            <dd className="break-all text-foreground">
              {selectedSnapshot.createdAt}
            </dd>
            <dt>task ID</dt>
            <dd className="break-all text-foreground">
              {selectedSnapshot.taskId}
            </dd>
          </dl>
          <details open={definitionBodyOpen || undefined} className="mt-2">
            <summary className="cursor-pointer font-mono text-xs text-foreground">
              固定定義本文を表示
            </summary>
            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all font-mono text-2xs">
              {JSON.stringify(selectedSnapshot.definition, null, 2)}
            </pre>
          </details>
        </>
      )}
    </section>
  )
}
