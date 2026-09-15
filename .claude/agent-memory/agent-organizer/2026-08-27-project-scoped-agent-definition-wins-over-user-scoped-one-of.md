### 2026-08-27 - Project-scoped agent definition wins over user-scoped one of the same name
- **Context**: Diagnostic run to determine which `agent-organizer.md` actually loads when
  `/home/david/.claude/agents/agent-organizer.md` (559 lines, generic, `model: sonnet`) and
  `/home/david/projects/GeeBeeAyy/.claude/agents/agent-organizer.md` (183 lines, GBA-specific,
  `model: opus`) both define `name: agent-organizer`.
- **Finding**: The loaded prompt is the project-scoped file. Verified by three markers absent
  from the user-scoped copy: the "Project context" section opening "**GeeBeeAyy!** - a Game Boy
  Advance emulator with a pixel bee theme."; the "Your position in the chain" section; and the
  roster table with a "Do not route here" column (rust-engineer, kotlin-specialist, swift-expert,
  mobile-developer, mobile-app-developer, search-specialist, accessibility-tester,
  visual-asset-generator). The Memory Protocol first bullet also differs and matched the project
  file: "- a routing decision that turned out wrong, and what the right lane was"
  (`/home/david/projects/GeeBeeAyy/.claude/agents/agent-organizer.md:17`) versus the user-scoped
  "- a constraint or behaviour that was not obvious from the code"
  (`/home/david/.claude/agents/agent-organizer.md:17`). No merge of the two occurs - the project
  file replaces the user one wholesale.
- **Application**: Edit only the project copy to change this agent's behaviour in GeeBeeAyy;
  edits to `~/.claude/agents/agent-organizer.md` are dead weight here and will silently take
  effect in any repo that lacks a project-scoped override. When a persona seems to ignore a
  recent instruction, check for a same-named file at the other scope before rewriting the prompt.
