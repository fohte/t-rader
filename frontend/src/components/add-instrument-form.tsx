import { useQueryClient } from '@tanstack/react-query'
import { Loader2, Plus } from 'lucide-react'
import { type FormEvent, useState } from 'react'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { $api } from '@/lib/api/client'

type AddInstrumentFormViewProps = {
  instrumentId: string
  name: string
  onInstrumentIdChange: (value: string) => void
  onNameChange: (value: string) => void
  onSubmit: (e: FormEvent) => void
  isSubmitting: boolean
  error: string | null
}

export function AddInstrumentFormView({
  instrumentId,
  name,
  onInstrumentIdChange,
  onNameChange,
  onSubmit,
  isSubmitting,
  error,
}: AddInstrumentFormViewProps) {
  return (
    <form onSubmit={onSubmit} className="space-y-2">
      <div className="flex items-end gap-2">
        <Input
          placeholder="銘柄コード (例: 7203)"
          value={instrumentId}
          onChange={(e) => onInstrumentIdChange(e.target.value)}
          disabled={isSubmitting}
          className="max-w-40"
        />
        <Input
          placeholder="銘柄名 (例: トヨタ自動車)"
          value={name}
          onChange={(e) => onNameChange(e.target.value)}
          disabled={isSubmitting}
        />
        <Button
          type="submit"
          disabled={!instrumentId.trim() || !name.trim() || isSubmitting}
        >
          {isSubmitting ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <Plus className="size-4" />
          )}
          追加
        </Button>
      </div>
      {error && <p className="text-sm text-destructive">{error}</p>}
    </form>
  )
}

type AddInstrumentFormProps = {
  watchlistId: string
  onNameRegistered: (instrumentId: string, name: string) => void
}

export function AddInstrumentForm({
  watchlistId,
  onNameRegistered,
}: AddInstrumentFormProps) {
  const [instrumentId, setInstrumentId] = useState('')
  const [name, setName] = useState('')
  const [error, setError] = useState<string | null>(null)
  const queryClient = useQueryClient()

  const addMutation = $api.useMutation('post', '/api/watchlists/{id}/items', {
    onSuccess: () => {
      onNameRegistered(instrumentId.trim(), name.trim())
      queryClient.invalidateQueries({
        queryKey: $api.queryOptions('get', '/api/watchlists/{id}/items', {
          params: { path: { id: watchlistId } },
        }).queryKey,
      })
      setInstrumentId('')
      setName('')
      setError(null)
    },
    onError: (err) => {
      // openapi-fetch のエラーから API のエラーメッセージを取得
      const message =
        (err as unknown as { error?: string })?.error ??
        '銘柄の追加に失敗しました'
      setError(message)
    },
  })

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    addMutation.mutate({
      params: { path: { id: watchlistId } },
      body: {
        instrument_id: instrumentId.trim(),
        name: name.trim(),
      },
    })
  }

  return (
    <AddInstrumentFormView
      instrumentId={instrumentId}
      name={name}
      onInstrumentIdChange={setInstrumentId}
      onNameChange={setName}
      onSubmit={handleSubmit}
      isSubmitting={addMutation.isPending}
      error={error}
    />
  )
}
