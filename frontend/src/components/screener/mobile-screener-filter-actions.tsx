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
      <button
        type="submit"
        className="w-full border border-primary bg-primary px-3 py-3 font-semibold text-primary-foreground hover:opacity-90"
      >
        この条件で絞り込む
      </button>
      {onAskAgent != null && (
        <button
          type="button"
          onClick={onAskAgent}
          className="flex w-full items-center justify-center gap-1 border border-primary bg-background px-3 py-3 font-semibold text-primary hover:bg-primary/10"
        >
          <span aria-hidden="true" className="font-bold">
            &gt;_
          </span>
          この条件でエージェントに頼む
        </button>
      )}
    </div>
  )
}
