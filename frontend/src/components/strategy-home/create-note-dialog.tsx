import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { CreateNoteDialogView } from '#components/strategy-home/create-note-dialog-view'
import { $api } from '#lib/api/client'

interface CreateNoteDialogProps {
  strategyId?: string
}

export function CreateNoteDialog({ strategyId }: CreateNoteDialogProps) {
  const [open, setOpen] = useState(false)
  const [title, setTitle] = useState('')
  const [body, setBody] = useState('')
  const [kind, setKind] = useState('')
  const [formError, setFormError] = useState<string | null>(null)
  const queryClient = useQueryClient()
  const noteKindsQuery = $api.useQuery('get', '/api/note-kinds', undefined, {
    enabled: open,
  })
  const createMutation = $api.useMutation('post', '/api/notes')

  function reset() {
    setTitle('')
    setBody('')
    setKind('')
    setFormError(null)
  }

  function handleOpenChange(nextOpen: boolean) {
    setOpen(nextOpen)
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
      ...(strategyId == null ? {} : { strategy_id: strategyId }),
    }
    createMutation.mutate(
      { body: requestBody },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/notes').queryKey,
          })
          reset()
          setOpen(false)
        },
        onError: () => {
          setFormError('ノートの作成に失敗しました')
        },
      },
    )
  }

  return (
    <CreateNoteDialogView
      open={open}
      onOpenChange={handleOpenChange}
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
