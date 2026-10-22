import { Button } from '@fohte/ui/button'
import { Plus } from 'lucide-react'

export function CreateStrategyCard({ onClick }: { onClick: () => void }) {
  return (
    <Button
      type="button"
      size="lg"
      onClick={() => {
        onClick()
      }}
      className="h-auto min-h-47 w-full flex-col"
    >
      <Plus className="size-6" aria-hidden="true" />
      <span>新しい戦略を作る</span>
    </Button>
  )
}
