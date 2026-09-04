import '#bootstrap'

<<<<<<< before updating
import { EnvError } from '#env'
import { main } from '#main'
||||||| last update
export const greet = (name: string): string => {
  return `Hello, ${name}!`
}
=======
import { err, ok, type Result } from 'neverthrow'
>>>>>>> after updating

<<<<<<< before updating
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
export const greet = (name: string): Result<string, Error> => {
  if (!name) return err(new Error('name must not be empty'))
  return ok(`Hello, ${name}!`)
}
>>>>>>> after updating
