import { NoteLinkRow } from '#components/notes/note-link-row'

interface RelatedNote {
  id: string
  title: string
  status: string
  strategyName: string
  author: string
  updatedAt: string
}

export function RelatedNotes({ notes }: { notes: RelatedNote[] }) {
  const recentNotes = [...notes].sort((left, right) =>
    right.updatedAt.localeCompare(left.updatedAt),
  )

  return (
    <section className="space-y-2" aria-labelledby="related-notes-title">
      <div className="flex items-baseline justify-between gap-3">
        <h2
          id="related-notes-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          関連ノート
        </h2>
        <span className="shrink-0 font-mono text-2xs text-muted-foreground">
          {notes.length} 件
        </span>
      </div>
      <div className="divide-y divide-border border border-border bg-card">
        {recentNotes.length === 0 ? (
          <p className="px-4 py-7 text-center font-mono text-xs text-muted-foreground">
            関連ノートはありません。
          </p>
        ) : (
          recentNotes.map((note) => (
            <NoteLinkRow
              key={note.id}
              noteId={note.id}
              title={note.title}
              status={note.status}
              strategyName={note.strategyName}
              author={note.author}
              updatedAt={note.updatedAt}
            />
          ))
        )}
      </div>
    </section>
  )
}
