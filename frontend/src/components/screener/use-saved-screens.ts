import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import type {
  SaveDialogState,
  SavedScreenDialogState,
} from '#components/screener/saved-screens-control-view'
import { $api } from '#lib/api/client'
import {
  getScreenerRangeError,
  type SavedScreen,
  type ScreenerSearch,
  toSavedScreenRequest,
} from '#lib/screener-search'

export function useSavedScreens({
  search,
  onSelect,
  onClearSelection,
}: {
  search: ScreenerSearch
  onSelect: (savedScreen: SavedScreen) => void
  onClearSelection: () => void
}) {
  const queryClient = useQueryClient()
  const listQuery = useQuery($api.queryOptions('get', '/api/saved-screens'))
  const createMutation = $api.useMutation('post', '/api/saved-screens')
  const updateMutation = $api.useMutation('put', '/api/saved-screens/{id}')
  const deleteMutation = $api.useMutation('delete', '/api/saved-screens/{id}')
  const [dialog, setDialog] = useState<SavedScreenDialogState | null>(null)
  const [error, setError] = useState<string>()
  const savedScreens = listQuery.data ?? []
  const isPending =
    createMutation.isPending ||
    updateMutation.isPending ||
    deleteMutation.isPending

  function invalidateList() {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/saved-screens').queryKey,
    })
  }

  function onDialogChange(nextDialog: SavedScreenDialogState | null) {
    setError(undefined)
    setDialog(nextDialog)
  }

  function onSave(
    name: string,
    saveDialog: SaveDialogState,
    filters: ScreenerSearch,
  ) {
    setError(undefined)
    if (saveDialog.type === 'rename') {
      updateMutation.mutate(
        {
          params: { path: { id: saveDialog.savedScreen.id } },
          body: {
            name: name.trim(),
            filters: saveDialog.savedScreen.filters,
            sort: saveDialog.savedScreen.sort,
          },
        },
        {
          onSuccess: () => {
            invalidateList()
            setDialog(null)
          },
          onError: (mutationError: unknown) => {
            setError(
              getSavedScreenErrorMessage(
                mutationError,
                '保存した条件の名前を変更できませんでした。',
              ),
            )
          },
        },
      )
      return
    }

    const rangeError = getScreenerRangeError(filters)
    if (rangeError != null) {
      setError(rangeError)
      return
    }

    createMutation.mutate(
      { body: toSavedScreenRequest(name, { ...search, ...filters }) },
      {
        onSuccess: (savedScreen) => {
          invalidateList()
          setDialog(null)
          onSelect(savedScreen)
        },
        onError: (mutationError: unknown) => {
          setError(
            getSavedScreenErrorMessage(
              mutationError,
              '保存した条件を作成できませんでした。',
            ),
          )
        },
      },
    )
  }

  function onOverwrite(filters: ScreenerSearch) {
    const savedScreen = savedScreens.find((item) => item.id === search.screen)
    if (savedScreen == null) return
    setError(undefined)
    const rangeError = getScreenerRangeError(filters)
    if (rangeError != null) {
      setError(rangeError)
      return
    }

    updateMutation.mutate(
      {
        params: { path: { id: savedScreen.id } },
        body: toSavedScreenRequest(savedScreen.name, { ...search, ...filters }),
      },
      {
        onSuccess: (updated) => {
          invalidateList()
          setDialog(null)
          onSelect(updated)
        },
        onError: (mutationError: unknown) => {
          setError(
            getSavedScreenErrorMessage(
              mutationError,
              '保存した条件を上書きできませんでした。',
            ),
          )
        },
      },
    )
  }

  function onDelete(savedScreen: SavedScreen) {
    setError(undefined)
    deleteMutation.mutate(
      { params: { path: { id: savedScreen.id } } },
      {
        onSuccess: () => {
          invalidateList()
          setDialog(null)
          if (search.screen === savedScreen.id) onClearSelection()
        },
        onError: (mutationError: unknown) => {
          setError(
            getSavedScreenErrorMessage(
              mutationError,
              '保存した条件を削除できませんでした。',
            ),
          )
        },
      },
    )
  }

  return {
    savedScreens,
    listPending: listQuery.isPending,
    listError: listQuery.isError,
    dialog,
    isPending,
    error,
    onDialogChange,
    onSave,
    onOverwrite,
    onDelete,
  }
}

function getSavedScreenErrorMessage(error: unknown, fallback: string): string {
  if (error != null && typeof error === 'object') {
    if ('error' in error && typeof error.error === 'string') return error.error
    if (
      'data' in error &&
      error.data != null &&
      typeof error.data === 'object' &&
      'error' in error.data &&
      typeof error.data.error === 'string'
    ) {
      return error.data.error
    }
  }
  return error instanceof Error && error.message !== ''
    ? error.message
    : fallback
}
