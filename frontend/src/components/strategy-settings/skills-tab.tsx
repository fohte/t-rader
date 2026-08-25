import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'

import { MarkdownEditor } from '#components/strategy-settings/markdown-editor'
import { validateSkillName } from '#components/strategy-settings/skill-name'
import { Button } from '#components/ui/button'
import { Input } from '#components/ui/input'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

interface SkillsTabProps {
  strategyId: string
}

export function SkillsTab({ strategyId }: SkillsTabProps) {
  const queryClient = useQueryClient()
  const { data, isPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}/skills',
    { params: { path: { id: strategyId } } },
  )

  const addSkill = $api.useMutation('put', '/api/strategies/{id}/skills/{name}')
  const saveSkill = $api.useMutation(
    'put',
    '/api/strategies/{id}/skills/{name}',
  )
  const deleteSkill = $api.useMutation(
    'delete',
    '/api/strategies/{id}/skills/{name}',
  )

  const [selected, setSelected] = useState<string | null>(null)
  const [newName, setNewName] = useState('')
  const [newNameError, setNewNameError] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)

  const skills = useMemo(() => data?.skills ?? {}, [data?.skills])
  const names = useMemo(() => Object.keys(skills).sort(), [skills])

  useEffect(() => {
    if (selected != null) return
    if (names.length === 0) return
    setSelected(names[0] ?? null)
  }, [selected, names])

  function invalidateSkills() {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/strategies/{id}/skills', {
        params: { path: { id: strategyId } },
      }).queryKey,
    })
  }

  function handleAdd() {
    const trimmed = newName.trim()
    const error = validateSkillName(trimmed)
    if (error != null) {
      setNewNameError(error)
      return
    }
    if (trimmed in skills) {
      setNewNameError('同名の skill が既に存在します')
      return
    }
    setNewNameError(null)
    addSkill.mutate(
      {
        params: { path: { id: strategyId, name: trimmed } },
        body: { content: '' },
      },
      {
        onSuccess: () => {
          invalidateSkills()
          setSelected(trimmed)
          setSaveError(null)
          setNewName('')
        },
        onError: () => {
          setNewNameError('skill の追加に失敗しました')
        },
      },
    )
  }

  function handleDelete(name: string) {
    if (!window.confirm(`skill "${name}" を削除しますか?`)) return
    deleteSkill.mutate(
      { params: { path: { id: strategyId, name } } },
      {
        onSuccess: () => {
          invalidateSkills()
          // useEffect の cleanup ロジックを使わず、削除元の場所で選択を解除する
          if (selected === name) {
            setSelected(null)
          }
        },
        onError: () => {
          setSaveError('skill の削除に失敗しました')
        },
      },
    )
  }

  function handleSaveContent(name: string, content: string) {
    setSaveError(null)
    saveSkill.mutate(
      {
        params: { path: { id: strategyId, name } },
        body: { content },
      },
      {
        onSuccess: () => {
          invalidateSkills()
        },
        onError: () => {
          setSaveError('保存に失敗しました')
        },
      },
    )
  }

  if (isPending) {
    return <Skeleton className="h-[320px] w-full" />
  }

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-[240px_minmax(0,1fr)]">
      <aside className="space-y-3">
        <div className="space-y-1.5">
          <label
            htmlFor="new-skill-name"
            className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
          >
            新しい skill
          </label>
          <div className="flex gap-2">
            <Input
              id="new-skill-name"
              value={newName}
              placeholder="skill 名"
              aria-invalid={newNameError != null}
              onChange={(e) => {
                setNewName(e.target.value)
                if (newNameError != null) setNewNameError(null)
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault()
                  handleAdd()
                }
              }}
            />
            <Button
              type="button"
              onClick={handleAdd}
              disabled={addSkill.isPending}
            >
              追加
            </Button>
          </div>
          {newNameError != null && (
            <p
              data-testid="new-skill-error"
              className="font-mono text-2xs text-primary"
            >
              {newNameError}
            </p>
          )}
        </div>

        <ul data-testid="skill-list" className="border border-border">
          {names.length === 0 && (
            <li className="px-3 py-2 font-mono text-xs text-muted-foreground">
              skill がまだありません
            </li>
          )}
          {names.map((name) => (
            <li
              key={name}
              className="flex items-center justify-between border-b border-border px-1 last:border-b-0"
            >
              <button
                type="button"
                onClick={() => {
                  setSelected(name)
                  setSaveError(null)
                }}
                data-active={selected === name}
                className="flex-1 truncate px-2 py-1.5 text-left font-mono text-[12.5px] hover:bg-surface-strong data-[active=true]:text-primary"
              >
                {name}
              </button>
              <button
                type="button"
                onClick={() => {
                  handleDelete(name)
                }}
                aria-label={`skill "${name}" を削除`}
                className="px-2 py-1 font-mono text-2xs text-muted-foreground hover:text-primary"
              >
                削除
              </button>
            </li>
          ))}
        </ul>
      </aside>

      <section>
        {selected == null ? (
          <p className="font-mono text-xs text-muted-foreground">
            左の一覧から skill を選択するか、新しく追加してください。
          </p>
        ) : (
          <MarkdownEditor
            // 選択 skill を切り替えるたびにエディタを完全に再初期化する
            key={selected}
            initialValue={skills[selected] ?? ''}
            isSaving={saveSkill.isPending}
            saveError={saveError}
            onSave={(next) => {
              handleSaveContent(selected, next)
            }}
          />
        )}
      </section>
    </div>
  )
}
