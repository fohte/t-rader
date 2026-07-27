<<<<<<< before updating
import '@/bootstrap'
||||||| last update
export const greet = (name: string): string => {
  return `Hello, ${name}!`
}
=======
import '#bootstrap'
>>>>>>> after updating

<<<<<<< before updating
import { EnvError } from '@/env'
import { main } from '@/main'

main().catch((err: unknown) => {
  if (err instanceof EnvError) {
    for (const issue of err.issues) console.error(issue)
  } else {
    console.error(err)
  }
  process.exit(1)
})
||||||| last update
export const greet = (name: string): string => {
  return `Hello, ${name}!`
}
=======
export const greet = (name: string): string => {
  return `Hello, ${name}!`
}
>>>>>>> after updating
