import { Skeleton } from '#components/ui/skeleton'

export type NoteVersionFallbackState = 'loading' | 'missing' | 'error'

interface NoteVersionFallbackProps {
  state: NoteVersionFallbackState
}

export function NoteVersionFallback({ state }: NoteVersionFallbackProps) {
  if (state === 'loading') {
    return (
      <div className="space-y-4">
        <Skeleton className="h-6 w-32" />
        <Skeleton className="h-10 w-2/3" />
        <Skeleton className="h-120 w-full" />
      </div>
    )
  }

  return (
    <div
      role={state === 'error' ? 'alert' : undefined}
      className={`border border-border bg-card px-4 py-4 font-mono text-sm ${state === 'error' ? 'text-primary' : 'text-muted-foreground'}`}
    >
      {state === 'error'
        ? 'バージョンを読み込めませんでした。'
        : 'ノートまたはバージョンが見つかりませんでした。'}
    </div>
  )
}
