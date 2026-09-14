### 2026-08-27 - Both CLAUDE.md files reach a subagent in full; the files they link to do not
- **Context**: Second diagnostic in the same run - which instruction files are actually injected
  into a subagent's context at spawn, answered from context alone rather than by reading files.
- **Finding**: One `<system-reminder>` block carries verbatim full copies of
  `/home/david/.claude/CLAUDE.md` (global, first) and `/home/david/projects/GeeBeeAyy/CLAUDE.md`
  (project, second) - headings, code fences and parentheticals intact, no summarisation. But the
  files those documents *link to* are not injected: `ROADMAP.md#who-owns-what` (cited at
  `/home/david/projects/GeeBeeAyy/CLAUDE.md` "Agents" section) and `.claude/memory.md` arrive only
  as pointers and cost a tool call each to read. This is why the roster table is duplicated into
  `/home/david/projects/GeeBeeAyy/.claude/agents/agent-organizer.md:40-49` rather than left as a
  ROADMAP link - the duplication is what makes it free at spawn.
- **Application**: Put doctrine a subagent must obey *inside* CLAUDE.md or inside the persona.
  Anything one hop away through a link is not in context and will be skipped by an agent that
  does not spend a read on it. Do not "fix" the roster duplication by replacing it with a link.
