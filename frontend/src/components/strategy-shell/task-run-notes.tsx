import { Link } from '@tanstack/react-router'

export type TaskRunNotesState = 'loading' | 'error' | 'success'

interface TaskRunNotesProps {
  notes: { id: string; title: string }[]
  state: TaskRunNotesState
}

export function TaskRunNotes({
  notes,
  state,
}: TaskRunNotesProps): React.ReactElement {
  const count =
    state === 'success' || (state === 'error' && notes.length > 0)
      ? `${notes.length.toLocaleString()} 件`
      : null

  return (
    <section
      aria-labelledby="task-run-notes-heading"
      aria-busy={state === 'loading'}
      className="space-y-2 border border-border bg-card p-4"
    >
      <div className="flex items-baseline justify-between border-b border-border pb-2">
        <h2
          id="task-run-notes-heading"
          className="font-mono text-xs text-muted-foreground-strong"
        >
          <span aria-hidden="true" className="text-foreground">
            &gt;
          </span>{' '}
          作成・更新したノート
        </h2>
        <span className="font-mono text-xs text-muted-foreground">
          {count ?? (state === 'loading' ? '取得中' : '取得失敗')}
        </span>
      </div>
      {state === 'loading' ? (
        <p role="status" className="font-mono text-xs text-muted-foreground">
          ノートを読み込み中です。
        </p>
      ) : state === 'error' ? (
        <p role="alert" className="font-mono text-xs text-muted-foreground">
          ノートの取得に失敗しました。
        </p>
      ) : notes.length === 0 ? (
        <p className="font-mono text-xs text-muted-foreground">
          この実行で作成・更新されたノートはありません。
        </p>
      ) : null}
      {notes.length > 0 && (
        <ul className="space-y-1">
          {notes.map((note) => (
            <li key={note.id}>
              <Link
                to="/notes/$noteId"
                params={{ noteId: note.id }}
                className="flex items-baseline gap-2 border border-border bg-background px-2.5 py-1.5 hover:border-muted-foreground"
              >
                <span className="flex-1 text-xs text-foreground">
                  {note.title}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
