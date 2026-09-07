# Parked agents

Personas kept in full but **not registered**, waiting for work this project
does not have.

They live here rather than in `.claude/agents/` because that is the only
reliable off switch: Claude Code scans `.claude/agents/` *and its
subdirectories*, and offers no setting or frontmatter field that disables an
agent in place. A nested `disabled/` folder would still be loaded.

Registering an agent for a stack the project does not use is worse than
useless - Claude picks an agent from the task plus its `description`, so a
persona that cannot help still competes for selection against one that can.

## Parked

_Nothing is parked right now._

`mobile-developer` used to sit here as a React Native and Flutter persona.
It was rewritten for this project's native stack on 2026-08-27 and moved into
`.claude/agents/`, where it owns device-level behaviour across both frontends:
storage and file access, lifecycle, battery and thermal, permissions. The
rejection of React Native and Flutter still stands - it is written into its
persona, not enforced by keeping the agent switched off.

## Reactivating one

Move the file back and start a new session:

```bash
mv .claude/agents-inactive/<name>.md .claude/agents/
```

Then rewrite its `description` to open with `Use PROACTIVELY` and to list
triggers from **this** project - real file paths, real symptoms - before it is
of any use. The stock description will not select correctly. Claude Code
builds its agent list when the session opens, so the agent only becomes
selectable in the next one.
