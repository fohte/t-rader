import { Skeleton } from '#components/ui/skeleton'

type State = 'loading' | 'error' | 'not-found'

export function StockPageState({ state }: { state: State }) {
  if (state === 'loading') {
    return (
      <main className="space-y-4" aria-label="銘柄ページを読み込み中">
        <Skeleton className="h-16 w-full" />
        <Skeleton className="h-110 w-full" />
      </main>
    )
  }

  return (
    <main className="space-y-2 py-8 text-center">
      <h1 className="font-mono text-lg font-bold">
        {state === 'not-found'
          ? '銘柄が見つかりません'
          : '銘柄ページを表示できません'}
      </h1>
      <p className="font-mono text-xs text-muted-foreground">
        {state === 'not-found'
          ? '銘柄コードを確認してください'
          : '時間をおいて再度お試しください'}
      </p>
    </main>
  )
}
