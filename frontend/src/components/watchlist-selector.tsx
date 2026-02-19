import { useQueryClient } from '@tanstack/react-query'
import { Loader2, Plus, Trash2 } from 'lucide-react'
import { type FormEvent, useState } from 'react'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type Watchlist = components['schemas']['Watchlist']

type WatchlistSelectorViewProps = {
  watchlists: Watchlist[]
  selectedId: string | null
  onSelect: (id: string | null) => void
  isCreating: boolean
  isDeleting: boolean
  onCreateSubmit: (name: string) => void
  onDelete: () => void
}

export function WatchlistSelectorView({
  watchlists,
  selectedId,
  onSelect,
  isCreating,
  isDeleting,
  onCreateSubmit,
  onDelete,
}: WatchlistSelectorViewProps) {
  const [isCreateMode, setIsCreateMode] = useState(false)
  const [newName, setNewName] = useState('')
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false)

  const handleCreateSubmit = (e: FormEvent) => {
    e.preventDefault()
    if (!newName.trim()) return
    onCreateSubmit(newName.trim())
    setNewName('')
    setIsCreateMode(false)
  }

  const selectedWatchlist = watchlists.find((w) => w.id === selectedId)

  return (
    <div className="flex items-center gap-2">
      {isCreateMode ? (
        <form onSubmit={handleCreateSubmit} className="flex items-center gap-2">
          <Input
            placeholder="ウォッチリスト名"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            disabled={isCreating}
            autoFocus
            className="max-w-60"
          />
          <Button
            type="submit"
            size="sm"
            disabled={!newName.trim() || isCreating}
          >
            {isCreating ? <Loader2 className="size-4 animate-spin" /> : '作成'}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => {
              setIsCreateMode(false)
              setNewName('')
            }}
          >
            キャンセル
          </Button>
        </form>
      ) : (
        <>
          <Select
            value={selectedId ?? undefined}
            onValueChange={(value) => onSelect(value)}
          >
            <SelectTrigger className="w-60">
              <SelectValue placeholder="ウォッチリストを選択" />
            </SelectTrigger>
            <SelectContent>
              {watchlists.map((w) => (
                <SelectItem key={w.id} value={w.id}>
                  {w.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <Button
            variant="outline"
            size="icon"
            onClick={() => setIsCreateMode(true)}
          >
            <Plus className="size-4" />
          </Button>

          {selectedId && (
            <Dialog
              open={isDeleteDialogOpen}
              onOpenChange={setIsDeleteDialogOpen}
            >
              <DialogTrigger asChild>
                <Button variant="outline" size="icon">
                  <Trash2 className="size-4" />
                </Button>
              </DialogTrigger>
              <DialogContent>
                <DialogHeader>
                  <DialogTitle>ウォッチリストの削除</DialogTitle>
                  <DialogDescription>
                    「{selectedWatchlist?.name}」を削除しますか?
                    この操作は取り消せません。
                  </DialogDescription>
                </DialogHeader>
                <DialogFooter>
                  <Button
                    variant="outline"
                    onClick={() => setIsDeleteDialogOpen(false)}
                  >
                    キャンセル
                  </Button>
                  <Button
                    variant="destructive"
                    disabled={isDeleting}
                    onClick={() => {
                      onDelete()
                      setIsDeleteDialogOpen(false)
                    }}
                  >
                    {isDeleting ? (
                      <Loader2 className="size-4 animate-spin" />
                    ) : (
                      '削除'
                    )}
                  </Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
          )}
        </>
      )}
    </div>
  )
}

type WatchlistSelectorProps = {
  watchlists: Watchlist[]
  selectedId: string | null
  onSelect: (id: string | null) => void
}

export function WatchlistSelector({
  watchlists,
  selectedId,
  onSelect,
}: WatchlistSelectorProps) {
  const queryClient = useQueryClient()

  const createMutation = $api.useMutation('post', '/api/watchlists', {
    onSuccess: (data) => {
      queryClient.invalidateQueries({
        queryKey: $api.queryOptions('get', '/api/watchlists').queryKey,
      })
      onSelect(data.id)
    },
  })

  const deleteMutation = $api.useMutation('delete', '/api/watchlists/{id}', {
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: $api.queryOptions('get', '/api/watchlists').queryKey,
      })
      // 削除後、残りの最初のウォッチリストを選択 (なければ null で解除)
      const remaining = watchlists.filter((w) => w.id !== selectedId)
      onSelect(remaining[0]?.id ?? null)
    },
  })

  return (
    <WatchlistSelectorView
      watchlists={watchlists}
      selectedId={selectedId}
      onSelect={onSelect}
      isCreating={createMutation.isPending}
      isDeleting={deleteMutation.isPending}
      onCreateSubmit={(name) => {
        createMutation.mutate({ body: { name } })
      }}
      onDelete={() => {
        if (selectedId) {
          deleteMutation.mutate({ params: { path: { id: selectedId } } })
        }
      }}
    />
  )
}
