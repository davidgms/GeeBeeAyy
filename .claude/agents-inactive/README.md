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

| Agent | Why it is parked |
|-------|------------------|
| `mobile-developer` | A React Native and Flutter persona - its own description names React Native 0.82+ and an 80% code-sharing target. This project rejects both **by design**: see "Why not Flutter/React Native" in `README.md`, since emulation needs raw audio buffers, direct GPU access and no interpreter between input and frame. The native lanes are covered by `kotlin-specialist`, `swift-expert` and `mobile-app-developer`. |

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
