interface NoteVersionChatActionProps {
  title: string
  onAsk: (title: string) => void
}

export function NoteVersionChatAction({
  title,
  onAsk,
}: NoteVersionChatActionProps) {
  return (
    <div className="flex flex-wrap items-center gap-2 border-t border-border pt-4 font-mono text-2xs text-muted-foreground">
      <span>このノートについて</span>
      <button
        type="button"
        aria-label={`「${title}」についてアナリストに聞く`}
        onClick={() => {
          onAsk(title)
        }}
        className="inline-flex items-center gap-1 border border-border px-2 py-0.5 text-muted-foreground-strong hover:border-primary hover:text-primary"
      >
        <span className="font-bold text-primary" aria-hidden="true">
          &gt;_
        </span>
        アナリストに聞く
      </button>
    </div>
  )
}
