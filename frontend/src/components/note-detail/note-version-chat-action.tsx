import { Button } from '@fohte/ui/button'

interface NoteVersionChatActionProps {
  title: string
  onAsk: (title: string) => void
}

export function NoteVersionChatAction({
  title,
  onAsk,
}: NoteVersionChatActionProps) {
  return (
    <Button
      type="button"
      variant="plain"
      aria-label={`「${title}」についてアナリストに聞く`}
      onClick={() => {
        onAsk(title)
      }}
    >
      <span className="inline-flex items-center gap-1">
        <span className="font-bold text-foreground" aria-hidden="true">
          &gt;_
        </span>
        アナリストに聞く
      </span>
    </Button>
  )
}
