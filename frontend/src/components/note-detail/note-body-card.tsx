import type { ReactNode } from 'react'

import { NOTE_BODY_INDENT_REM } from '#components/note-detail/markdown-body-range-comment-context'

export const NOTE_BODY_GUTTER_CARD_PADDING_REM = 2.25
export const NOTE_BODY_GUTTER_CARD_BLEED_REM =
  NOTE_BODY_GUTTER_CARD_PADDING_REM - NOTE_BODY_INDENT_REM

interface NoteBodyCardProps {
  children: ReactNode
}

export function NoteBodyCard({ children }: NoteBodyCardProps) {
  return (
    <article
      className="space-y-4 border border-border bg-card px-5 py-5 text-foreground"
      style={{
        marginLeft: `-${String(NOTE_BODY_GUTTER_CARD_BLEED_REM)}rem`,
        paddingLeft: `${String(NOTE_BODY_GUTTER_CARD_PADDING_REM)}rem`,
      }}
    >
      {children}
    </article>
  )
}
