import { Button } from '@fohte/ui/button'

export function MobileScreenerFilterActions({
  validationError,
  onAskAgent,
}: {
  validationError?: string
  onAskAgent?: () => void
}) {
  return (
    <div
      role="group"
      aria-label="条件の操作"
      className="mt-4 shrink-0 space-y-2 border-t border-border pt-4"
    >
      {validationError != null && (
        <p role="alert" className="text-2xs text-destructive">
          {validationError}
        </p>
      )}
      <Button type="submit" className="w-full">
        この条件で絞り込む
      </Button>
      {onAskAgent != null && (
        <Button
          type="button"
          variant="outline"
          onClick={onAskAgent}
          className="w-full"
        >
          <span aria-hidden="true" className="font-bold">
            &gt;_
          </span>
          この条件でエージェントに頼む
        </Button>
      )}
    </div>
  )
}
