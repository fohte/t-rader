import { PopoverContent as SharedPopoverContent } from '@fohte/ui/popover'
import * as React from 'react'

function PopoverContent({
  align = 'center',
  ...props
}: React.ComponentProps<typeof SharedPopoverContent>) {
  return <SharedPopoverContent align={align} {...props} />
}

export { Popover, PopoverTrigger } from '@fohte/ui/popover'
export { PopoverContent }
