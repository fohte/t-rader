import type { ReactNode } from 'react'

export const NOTE_BODY_GUTTER_CARD_PADDING_REM = 2.25

interface NoteBodyCardProps {
  children: ReactNode
}

export function NoteBodyCard({ children }: NoteBodyCardProps) {
  return (
    <article
      className="space-y-4 border border-border bg-card px-5 py-5 text-foreground"
      style={{
        paddingLeft: `${String(NOTE_BODY_GUTTER_CARD_PADDING_REM)}rem`,
      }}
    >
      {children}
    </article>
  )
}
