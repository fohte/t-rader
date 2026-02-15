import type { FallbackProps } from 'react-error-boundary'

export function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
  return (
    <div className="flex h-screen items-center justify-center">
      <div className="max-w-md text-center">
        <h1 className="text-2xl font-bold text-destructive">
          エラーが発生しました
        </h1>
        <p className="mt-2 text-muted-foreground">
          {error instanceof Error
            ? error.message
            : '予期しないエラーが発生しました'}
        </p>
        <button
          type="button"
          onClick={resetErrorBoundary}
          className="mt-4 rounded-md bg-primary px-4 py-2 text-primary-foreground hover:bg-primary/90"
        >
          再試行
        </button>
      </div>
    </div>
  )
}
