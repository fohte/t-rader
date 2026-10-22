import { Button } from '@fohte/ui/button'
import type { ReactNode } from 'react'

interface MarkdownBodyGraphNoticeProps {
  children: ReactNode
  onRetry?: () => void
}

export function MarkdownBodyGraphNotice({
  children,
  onRetry,
}: MarkdownBodyGraphNoticeProps) {
  return (
    <div
      role="alert"
      className="my-3 border border-dashed border-border p-3 font-mono text-xs text-muted-foreground"
    >
      {children}
      {onRetry == null ? null : (
        <span className="ml-2 underline">
          <Button type="button" variant="plain" onClick={onRetry}>
            再試行
          </Button>
        </span>
      )}
    </div>
  )
}
