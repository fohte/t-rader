import { Checkbox } from '@fohte/ui/checkbox'

import type { components } from '#lib/api/schema.gen'

type WarningDisposition = components['schemas']['WarningDispositionResponse']

export interface WarningDispositionReviewProps {
  versionNo?: number
  warningDispositions: WarningDisposition[]
  confirmation?: {
    checked: boolean
    onChange: (checked: boolean) => void
  }
}

function formatLocation(location: WarningDisposition['location']): string {
  if (location.kind === 'body') {
    return `本文 byte ${String(location.start)}–${String(location.end)}`
  }
  return `図 ${location.path} byte ${String(location.start)}–${String(location.end)}`
}

export function WarningDispositionReview({
  versionNo,
  warningDispositions,
  confirmation,
}: WarningDispositionReviewProps) {
  if (warningDispositions.length === 0) return null

  return (
    <section className="space-y-3 border border-status-rejected/40 bg-status-rejected/5 p-3">
      <div className="space-y-2">
        <h3 className="font-mono text-xs font-bold text-foreground">
          理由付きで無視した警告 ({String(warningDispositions.length)} 件)
          {versionNo != null && ` · v${String(versionNo)}`}
        </h3>
        <ul className="space-y-2">
          {warningDispositions.map((disposition) => (
            <li
              key={`${disposition.warning_id}:${disposition.location.kind}:${String(disposition.location.start)}`}
              className="space-y-1 border-l-2 border-status-rejected/50 pl-2 text-xs"
            >
              <p className="break-words text-foreground">
                対象: <code>{disposition.value}</code>
              </p>
              <p className="break-all text-muted-foreground">
                {formatLocation(disposition.location)}
              </p>
              <p className="break-words text-muted-foreground-strong">
                理由: {disposition.reason}
              </p>
            </li>
          ))}
        </ul>
      </div>
      {confirmation != null && (
        <label className="flex items-start gap-2 border-t border-border pt-3 text-xs text-foreground">
          <Checkbox
            checked={confirmation.checked}
            onCheckedChange={(checked) => {
              confirmation.onChange(checked)
            }}
            className="mt-0.5 shrink-0"
          />
          <span>理由と対象箇所を確認し、数値の例外として承認します</span>
        </label>
      )}
    </section>
  )
}
