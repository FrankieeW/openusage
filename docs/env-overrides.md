# Environment Variables

The Env page (nav icon: braces, labelled **Environment Variables**) lets you
define variables that plugins can read, without exporting them into your shell.

Variables are grouped into **cards**. Tick the **Active** box on a card to make
its variables visible to plugins; uncheck to ignore it.

## Value Syntax

The value of an entry is parsed by its first character:

- `api` — literal. The plugin receives the string `api`.
- `$ZAI_API_KEY` — reference. The plugin receives the value of `ZAI_API_KEY` from
  your real shell environment.
- `$$$HOME` — literal that begins with `$`. The two leading `$$` are consumed
  and the rest is stored verbatim, so this stores the literal `$HOME` (not a
  reference).

## Conflicts

If two or more active cards define the same variable name, plugins see
`[CONFLICT: NAME]` for that name instead of a value. Resolve the conflict by
disabling one of the cards, or by renaming the variable in one of them.

## Storage

Cards are persisted to `env.json` in the app's data directory. Each card stores
its own `enabled` flag; active cards are the only cards sent to plugins.

Older files that still contain `activeGroupIds` are migrated automatically.
During that migration, `activeGroupIds` is used once to update each card's
`enabled` flag, then the old key is removed and `envSchemaVersion` is set to
`2`. Restart OpenUsage after editing the file by hand.

## Learn More

For full design notes, see the
[design spec](https://github.com/FrankieeW/openusage/blob/main/docs/superpowers/specs/2026-06-15-env-overrides-page-design.md)
in the repo.
