import { useEffect, useState } from 'react'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'

export function CashBalanceDialog({
  open,
  initial,
  onOpenChange,
  onSave,
}: {
  open: boolean
  initial: number
  onOpenChange: (v: boolean) => void
  onSave: (cash: number) => void
}) {
  const [value, setValue] = useState<string>(String(initial))

  useEffect(() => {
    if (open) setValue(String(initial))
  }, [open, initial])

  const numeric = Number(value)
  const valid = value !== '' && Number.isFinite(numeric) && numeric >= 0

  const submit = () => {
    if (!valid) return
    onSave(Math.round(numeric))
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>現金残高を更新</DialogTitle>
          <DialogDescription>
            ポートフォリオ全体の現金 (円) を入力します。MVP
            ではブラウザのローカルストレージに保存します。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-2 py-2">
          <label
            htmlFor="cash-balance-input"
            className="block font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
          >
            現金 (¥)
          </label>
          <Input
            id="cash-balance-input"
            type="number"
            inputMode="numeric"
            min={0}
            value={value}
            onChange={(e) => {
              setValue(e.target.value)
            }}
            placeholder="1000000"
          />
        </div>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              onOpenChange(false)
            }}
          >
            キャンセル
          </Button>
          <Button type="button" disabled={!valid} onClick={submit}>
            保存
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
