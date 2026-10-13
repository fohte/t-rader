import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'

import { CreateNoteDialogView } from '#components/strategy-home/create-note-dialog-view'
import { $api } from '#lib/api/client'

export function CreateNoteDialog({
  open,
  onOpenChange,
  initialTitle = '',
  initialBody = '',
  showTrigger = true,
  onCreated,
}: {
  open?: boolean
  onOpenChange?: (open: boolean) => void
  initialTitle?: string
  initialBody?: string
  showTrigger?: boolean
  onCreated?: (noteId: string) => void
} = {}) {
  const [localOpen, setLocalOpen] = useState(false)
  const dialogOpen = open ?? localOpen
  const previousOpen = useRef(false)
  const [title, setTitle] = useState('')
  const [body, setBody] = useState('')
  const [kind, setKind] = useState('')
  const [formError, setFormError] = useState<string | null>(null)
  const queryClient = useQueryClient()
  const noteKindsQuery = $api.useQuery('get', '/api/note-kinds', undefined, {
    enabled: open,
  })
  const createMutation = $api.useMutation('post', '/api/notes')

  useEffect(() => {
    if (dialogOpen && !previousOpen.current) {
      setTitle(initialTitle)
      setBody(initialBody)
      setKind('')
      setFormError(null)
    }
    previousOpen.current = dialogOpen
  }, [dialogOpen, initialBody, initialTitle])

  function reset() {
    setTitle('')
    setBody('')
    setKind('')
    setFormError(null)
  }

  function handleOpenChange(nextOpen: boolean) {
    if (open == null) setLocalOpen(nextOpen)
    onOpenChange?.(nextOpen)
    if (!nextOpen) reset()
  }

  function handleSubmit(e: React.SyntheticEvent) {
    e.preventDefault()
    if (createMutation.isPending) return

    const trimmedTitle = title.trim()
    const trimmedBody = body.trim()
    if (trimmedTitle === '' || trimmedBody === '') {
      setFormError('タイトルと本文は必須です')
      return
    }

    const requestBody = {
      title: trimmedTitle,
      body_md: trimmedBody,
      kind: kind === '' ? null : kind,
    }
    createMutation.mutate(
      { body: requestBody },
      {
        onSuccess: (note) => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/notes').queryKey,
          })
          onCreated?.(note.id)
          reset()
          handleOpenChange(false)
        },
        onError: () => {
          setFormError('ノートの作成に失敗しました')
        },
      },
    )
  }

  return (
    <CreateNoteDialogView
      open={dialogOpen}
      onOpenChange={handleOpenChange}
      showTrigger={showTrigger}
      title={title}
      body={body}
      kind={kind}
      noteKinds={noteKindsQuery.data ?? []}
      noteKindsPending={noteKindsQuery.isPending}
      noteKindsError={noteKindsQuery.isError}
      formError={formError}
      isSubmitting={createMutation.isPending}
      onTitleChange={setTitle}
      onBodyChange={setBody}
      onKindChange={setKind}
      onSubmit={handleSubmit}
    />
  )
}
