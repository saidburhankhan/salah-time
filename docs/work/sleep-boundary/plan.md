# Plan: sleep-boundary

## Outcome

After Isha, Salah counts down to a user-configured sleep time so the widget
communicates the end of the productive day. After that boundary it clearly
counts down to Fajr as sleeping time.

## Decisions

- Sleep is one local wall-clock boundary, not a duration or wake-up time.
- Sleep is a planning boundary and must not be represented as a prayer.
- Keep the existing `next` JSON field for compatibility, adding boundary kind
  and current-block information for future task planning.
- The default sleep boundary is `23:00`; configure it with `--sleep-time HH:MM`.
- History persistence will use a domain/repository boundary and portable,
  versioned exports in a later work item; this slice will not create temporary
  history storage.

## Non-goals

- Tasks, routines, SQLite history, import/export, cloud sync, and the full app.
- Automatic sleep detection or a wake-up setting.

## Slices

- [x] S1 — configuration and boundary-selection domain contract
- [x] S2 — status JSON, CLI, and widget presentation
- [ ] S3 — integration and live verification

## Acceptance checks

- [x] After Isha and before sleep, `next` is the sleep boundary.
- [x] After sleep and before Fajr, `next` is Fajr and the current block is sleep.
- [x] Legacy configuration loads with the default sleep time.
- [x] Invalid sleep times cannot overwrite valid configuration.
- [x] Focused tests pass.
- [x] `./scripts/test` passes.
- [x] Strict Clippy passes.
- [x] Installed plugin validates (live after-Isha/sleeping view still to observe).
- [x] A bedtime after midnight is the next boundary after Isha.
- [x] The popup keeps Sunrise.
