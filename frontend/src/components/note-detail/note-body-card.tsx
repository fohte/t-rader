import type { ReactNode } from 'react'

export const NOTE_BODY_GUTTER_CARD_PADDING_REM = 2.25

interface NoteBodyCardProps {
  children: ReactNode
}

export function NoteBodyCard({ children }: NoteBodyCardProps) {
  return (
    <article className="space-y-4 border border-border bg-card py-5 pr-5 pl-9 text-foreground">
      {children}
    </article>
  )
}
