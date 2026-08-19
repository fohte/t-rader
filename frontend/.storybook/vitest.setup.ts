import * as addonAnnotations from '@fohte/storybook-addon/preview'
import { setProjectAnnotations } from '@storybook/react-vite'

import * as previewAnnotations from './preview'

setProjectAnnotations([addonAnnotations, previewAnnotations])
