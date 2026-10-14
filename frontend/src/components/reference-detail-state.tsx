type ReferenceDetailStateProps = {
  state: 'loading' | 'not-found' | 'error'
  label: string
  identifier: string
}

export function ReferenceDetailState(props: ReferenceDetailStateProps) {
  if (props.state === 'loading') {
    return (
      <main className="space-y-5 font-sans text-foreground" aria-busy="true">
        <div className="space-y-3">
          <div className="h-8 w-48 animate-pulse bg-surface-strong" />
          <div className="h-9 w-36 animate-pulse bg-surface-strong" />
        </div>
        <p className="font-mono text-xs text-muted-foreground">
          {props.label}を読み込んでいます: {props.identifier}
        </p>
      </main>
    )
  }

  if (props.state === 'not-found') {
    return (
      <main className="space-y-2 font-sans text-foreground">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          {props.label}が見つかりません
        </h1>
        <p className="font-mono text-sm text-muted-foreground">
          {props.identifier}
        </p>
      </main>
    )
  }

  return (
    <main className="space-y-2 font-sans text-foreground">
      <h1 className="text-2xl font-bold leading-tight tracking-tight">
        {props.label}を読み込めませんでした
      </h1>
      <p className="font-mono text-sm text-muted-foreground">
        {props.identifier}
      </p>
    </main>
  )
}
