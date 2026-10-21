import { Tabs, TabsList, TabsPanel, TabsTab } from '@fohte/ui/tabs'
import { Plus } from 'lucide-react'

import {
  IndicatorEditor,
  type IndicatorEditorValue,
  type IndicatorScopeLabel,
  type PreviewState,
} from '#components/indicators/indicator-editor'
import { Skeleton } from '#components/ui/skeleton'

interface IndicatorListItem {
  id: string
  name: string
}

interface IndicatorsPageViewProps {
  scope: IndicatorScopeLabel
  indicators: IndicatorListItem[]
  isPending: boolean
  selectedId: string | null
  isCreating: boolean
  initial: IndicatorEditorValue
  isSaving: boolean
  saveError: string | null
  preview: PreviewState
  onSelectIndicator: (target: { id: string | null; creating: boolean }) => void
  onSave: (next: IndicatorEditorValue) => void
  onPreview: (args: {
    code: string
    inputSchema: string
    outputSchema: string
    argsJson: string
  }) => void
}

export function IndicatorsPageView({
  scope,
  indicators,
  isPending,
  selectedId,
  isCreating,
  initial,
  isSaving,
  saveError,
  preview,
  onSelectIndicator,
  onSave,
  onPreview,
}: IndicatorsPageViewProps) {
  const activeTab = isCreating
    ? 'create'
    : selectedId == null
      ? ''
      : `indicator:${selectedId}`

  function handleTabChange(value: string) {
    if (value === 'create') {
      onSelectIndicator({ id: null, creating: true })
      return
    }
    if (value.startsWith('indicator:')) {
      onSelectIndicator({
        id: value.slice('indicator:'.length),
        creating: false,
      })
    }
  }

  return (
    <Tabs value={activeTab} onValueChange={handleTabChange}>
      <div className="space-y-5">
        <div className="flex flex-wrap items-center gap-2">
          {isPending && <Skeleton className="h-8 w-30" />}
          <TabsList
            data-testid={isPending ? undefined : 'indicator-list'}
            aria-label="Indicators"
          >
            {indicators.map((indicator) => (
              <TabsTab
                key={indicator.id}
                value={`indicator:${indicator.id}`}
                title={indicator.name}
              >
                <span className="block max-w-48 truncate">
                  {indicator.name}
                </span>
              </TabsTab>
            ))}
            <TabsTab value="create">
              <span className="inline-flex items-center gap-1.5">
                <Plus className="size-3.5" aria-hidden="true" />
                新規 indicator
              </span>
            </TabsTab>
          </TabsList>
        </div>

        {activeTab === '' ? (
          <div className="font-mono text-xs text-muted-foreground">
            indicator を選択するか、「新規 indicator」を押してください。
          </div>
        ) : (
          <TabsPanel value={activeTab}>
            <IndicatorEditor
              key={isCreating ? 'new' : selectedId}
              scope={scope}
              initial={initial}
              nameReadOnly={!isCreating}
              onSave={onSave}
              isSaving={isSaving}
              saveError={saveError}
              onPreview={onPreview}
              preview={preview}
            />
          </TabsPanel>
        )}
      </div>
    </Tabs>
  )
}
