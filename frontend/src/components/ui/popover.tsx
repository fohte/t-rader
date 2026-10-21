import { PopoverContent as SharedPopoverContent } from '@fohte/ui/popover'
import * as React from 'react'

function PopoverContent({
  side = 'bottom',
  sideOffset = 4,
  align = 'center',
  alignOffset = 0,
  ...props
}: React.ComponentProps<typeof SharedPopoverContent>) {
  return (
    <SharedPopoverContent
      side={side}
      sideOffset={sideOffset}
      align={align}
      alignOffset={alignOffset}
      {...props}
    />
  )
}

export { Popover, PopoverTrigger } from '@fohte/ui/popover'
export { PopoverContent }
