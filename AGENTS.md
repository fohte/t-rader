# AGENTS.md

## Code organization rules

### Split files before they grow past ~500 lines of production code

When a change would push a file's non-test code past ~500 lines, split it along responsibility seams before adding more. Splits must be move-only commits: no logic changes, renames, or reformatting mixed in. Keep external import paths unchanged by keeping the entrypoint file in place and re-exporting the pieces you split out into new files (e.g. `foo.rs` gains a `foo/` directory for its submodules, `index.ts` re-exports from the new files). Tests move together with the code they verify.

Prefer creating a new focused file over appending to the largest existing one.

## Error handling rules

### Return a `Result` instead of throwing

`errorHandling` in `eslint.config.js` bans `throw`/`try-catch` in production code and requires every returned `Result` to be consumed (`no-restricted-syntax`, `neverthrow/must-use-result` in `@fohte/eslint-config`). Return a `Result`/`ResultAsync` from [neverthrow](https://github.com/supermacro/neverthrow) instead (add it as a dependency first if this package doesn't have it yet):

```ts
// bad: throws
function parseConfig(raw: string): Config {
  if (!isValid(raw)) throw new Error('invalid config')
  return JSON.parse(raw)
}

// good: returns a Result
function parseConfig(raw: string): Result<Config, ConfigError> {
  if (!isValid(raw)) return err(new ConfigError('invalid config'))
  return ok(JSON.parse(raw))
}
```

Use `ResultAsync.fromPromise()` or `Result.fromThrowable()` to interop with a throwing API without a local try/catch. If the throw-based contract genuinely can't be wrapped that way, catch the exception, wrap it in a `BoundaryError` subclass (see `src/errors.ts`), and rethrow it — `no-restricted-syntax` bans `try`/`throw` as separate selectors, so both the `try` and the `throw` need their own `eslint-disable-next-line no-restricted-syntax` comment explaining why.

## Storybook

### Write a story for every presentational component

Every presentational component under `src/components/` should have a co-located `.stories.tsx` file matching the component's filename (e.g. `src/components/card.tsx` pairs with `src/components/card.stories.tsx`). If a source file exports multiple components, give each one its own `<component-name>.stories.tsx` file instead of matching the source filename. Write one story per meaningful state/variant of the component.

### Extract route-inline UI that has its own appearance or state

Stories are the only thing the `vrt` CI check renders and screenshots. A route file is never rendered by a story, so UI written inline in a route — a `<select>`, a checkbox, a column header, an empty state, a full-screen loading/not-found view — has no visual-regression coverage even when the rule above (every presentational component under `src/components/` has a story) is fully satisfied.

Keep in the route file: data fetching, URL parameter handling, and composing already-extracted, already-storied components into the screen layout. Extract into `src/components/` (with a story) anything that has its own visual appearance or state, even a few lines of JSX, since a story is the only way it gets checked for a visual regression.

### A story is a prop-driven visual state, not a behavior test

A story's args fully determine what renders; the story never clicks, types, or otherwise interacts to reach that state. `fohte/no-play-in-stories` (`@fohte/eslint-config`) rejects a `play` function on a story or its `meta`, and rejects `parameters.screenshot.skip`, because the `vrt` check screenshots every story as rendered — an interaction-driven story produces whatever the `play` function happens to leave on screen at screenshot time, not a stable state.

For a state that would otherwise take interaction to reach — an open menu/popover/dialog, a field mid-edit — expose it through props (e.g. `open`/`defaultOpen`) instead of driving it with a `play` function, adding the prop or extracting a presentational subcomponent if the component doesn't already support it. Test the interaction itself (click, type, assert) in a co-located `.test.tsx` file, not in a story.

### Prefer Storybook over manual browser checks

When you need to check how a component looks in a given state, write or update its story and verify it with `cd frontend && pnpm run storybook:screenshot -- --changed origin/main` (swap `origin/main` for this repo's default branch if it differs) instead of starting a dev server and driving a browser manually. Dropping the ref limits `--changed` to staged/unstaged files only, so it silently runs nothing once the change is committed. The `vrt` CI check already renders and diffs every story on every PR, so this scoped run is enough — running the full `storybook:screenshot` suite instead keeps a headless Chromium instance (a multi-process browser, not a single lightweight process) busy per worker for as long as it takes to get through every story, competing with any other concurrent session or worktree for the same machine's CPU and memory.

## Visual Regression Testing (VRT)

### Understand why the `vrt` check fails

The `vrt` CI check renders Storybook stories to screenshots and compares them against the `main` baseline with reg-suit. A failure ("Visual differences detected") means the pixel diff exceeded reg-suit's `matchingThreshold`, not that something is broken — open the reg-suit report link posted on the PR and compare the actual/expected/diff images to judge whether the change is intentional.

### Never add the `vrt-approved` label yourself

The `vrt-approval` workflow treats the `vrt-approved` label as confirmation that a human reviewed the diff images and approved them. Only a human can make that visual judgment, so after inspecting the diff, ask the user to review it and add the label themselves — do not add it yourself even if the diff looks correct.

## Test code rules

### Assert on the whole output with a single equality check

Treat each test as a spec: build the expected output as one literal value (object, struct, JSON, array, etc.) and compare it to the actual output with a single equality assertion. Do not split the assertion into per-field checks, and do not use partial matchers (substring contains, `toContain`, `toMatchObject`, prefix/suffix checks, regex-on-substring, etc.). Partial matches silently ignore unexpected fields and extra elements, so the test stops working as a spec the moment the shape of the output changes.

```ts
// bad: picks fields one by one — silent on any new/changed field
const ev = run()
expect(ev.path).toBe('/a')
expect(ev.event).toBe('ok')
expect(ev.message).toContain('done')

// good: one literal, one equality — any drift in shape fails the test
expect(run()).toEqual({
  path: '/a',
  event: 'ok',
  message: 'done',
})
```

```rust
// bad
let ev = run();
assert_eq!(ev["path"], "/a");
assert_eq!(ev["event"], "ok");
assert!(ev["message"].as_str().unwrap().contains("done"));

// good
assert_eq!(
    run(),
    json!({
        "path": "/a",
        "event": "ok",
        "message": "done",
    }),
);
```

For dynamic fields (timestamps, UUIDs, random IDs), normalize them in a helper before the comparison (e.g. replace with a fixed placeholder) so the full output can still be asserted in one equality check. Do not weaken the assertion to dodge the dynamic value.

The `no-assert-contains` ast-grep rule rejects `assert!(x.contains(...))` at the expression level; this guideline is the broader principle that the rule is one instance of.

### Parameterize similar test cases with rstest

Do not write multiple test functions that differ only in input/expected values. Use `#[rstest]` with `#[case]`.

```rust
// bad: separate functions per case
#[test]
fn test_parse_empty() { assert_eq!(parse(""), None); }
#[test]
fn test_parse_valid() { assert_eq!(parse("hello"), Some("hello")); }

// good: parameterized
#[rstest]
#[case::empty("", None)]
#[case::valid("hello", Some("hello"))]
fn test_parse(#[case] input: &str, #[case] expected: Option<&str>) {
    assert_eq!(parse(input), expected);
}
```

### Always name `#[case]` variants

Use `#[case::descriptive_name(...)]`, not bare `#[case(...)]`. Named cases identify failures without inspecting values.

### Use `#[fixture]` for shared test setup

Do not repeat the same setup code across tests. Extract into `#[fixture]`.

```rust
// bad: duplicated setup
#[rstest]
fn test_a() { let repo = make_repo(); /* ... */ }
#[rstest]
fn test_b() { let repo = make_repo(); /* ... */ }

// good: fixture injection
#[fixture]
fn repo() -> Repo { make_repo() }
#[rstest]
fn test_a(repo: Repo) { /* ... */ }
```

### Use `indoc!` for multiline string literals in tests

Do not embed `\n` in string literals. Use `indoc!` for readability.

### Extract repeated assertions into helper functions

If the same assertion chain appears in 3+ tests, extract it into a helper.

### Do not write tests that only verify test helpers

Tests must verify production code. Tests that only assert on test helpers, fixtures, or mocks are unnecessary. Remove them.
