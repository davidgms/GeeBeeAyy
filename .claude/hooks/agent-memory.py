#!/usr/bin/env python3
"""Enforce the agent Memory Protocol.

Wired to two hook events:

  SubagentStart  -> stamps the moment this subagent began
  SubagentStop   -> checks whether the subagent wrote to its own agent file
                    before finishing; nudges once if it did not.

Why this exists: the Memory Protocol sits at the tail of every agent
definition and gets skipped in practice. An instruction competing with "write
the final report" loses every time, so this makes the check deterministic
instead. Brought over from the rpg-llm project, where the protocol went unused
for weeks until the check was enforced by a hook.

Payload (per the SubagentStart/SubagentStop hook contract):
    { "agent_id": ..., "agent_type": ..., "agent_transcript_path": ... }

Exit codes: 0 allows the subagent to finish; 2 sends stderr back to the
subagent so it can record its discoveries and then stop.
"""

import json
import os
import pathlib
import sys

# temp/ is gitignored and survives reboots - see CLAUDE.md.
STAMP_DIR = pathlib.Path("temp/.agent-stamps")

# Agents with no definition file we could write to (built-ins).
NO_FILE_OK = {"general-purpose", "Explore", "Plan", "claude", "fork", "statusline-setup"}


def project_dir() -> pathlib.Path:
    return pathlib.Path(os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd()))


def agent_file(root: pathlib.Path, agent_type: str) -> pathlib.Path | None:
    """Where this agent is expected to write.

    Agents declaring `memory: project` get a native directory that Claude Code
    auto-loads on every spawn; that is the target for them. Agents still on the
    older convention write to their own definition's `## Discoveries` section.
    Project agents win over global ones, matching Claude Code's resolution.
    """
    for scope, base in (
        ("project", root / ".claude" / "agent-memory"),
        ("local", root / ".claude" / "agent-memory-local"),
        ("user", pathlib.Path.home() / ".claude" / "agent-memory"),
    ):
        native = base / agent_type / "MEMORY.md"
        if native.is_file():
            return native
    for candidate in (
        root / ".claude" / "agents" / f"{agent_type}.md",
        pathlib.Path.home() / ".claude" / "agents" / f"{agent_type}.md",
    ):
        if candidate.is_file():
            return candidate
    return None


def newest_mtime(target: pathlib.Path) -> float:
    """A native memory dir can grow topic files beside MEMORY.md."""
    if target.name != "MEMORY.md":
        return target.stat().st_mtime
    return max(p.stat().st_mtime for p in target.parent.glob("*.md"))


def main() -> int:
    event = sys.argv[1] if len(sys.argv) > 1 else ""
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        return 0  # never break the run over a malformed payload

    agent_id = str(payload.get("agent_id") or "").replace("/", "_")
    agent_type = str(payload.get("agent_type") or "")
    if not agent_id:
        return 0

    root = project_dir()
    stamps = root / STAMP_DIR
    stamps.mkdir(parents=True, exist_ok=True)
    stamp = stamps / agent_id
    blocked = stamps / f"{agent_id}.blocked"

    if event == "SubagentStart":
        stamp.touch()
        return 0

    if event != "SubagentStop":
        return 0

    # From here on we are deciding whether to let the subagent finish.
    def cleanup() -> None:
        for path in (stamp, blocked):
            path.unlink(missing_ok=True)

    if agent_type in NO_FILE_OK:
        cleanup()
        return 0

    target = agent_file(root, agent_type)
    if target is None or not stamp.exists():
        # Unknown agent, or we never saw it start - nothing to judge against.
        cleanup()
        return 0

    if newest_mtime(target) > stamp.stat().st_mtime:
        cleanup()  # it recorded something
        return 0

    if blocked.exists():
        cleanup()  # already nudged once; do not loop
        return 0

    blocked.touch()
    rel = target.relative_to(root) if target.is_relative_to(root) else target
    sys.stderr.write(
        f"Memory Protocol: you did not write to {rel} during this task.\n\n"
        "Before finishing, append an entry to that file's `## Discoveries` "
        "section for anything you learned that a future instance of yourself "
        "would otherwise have to re-derive: a pattern, a constraint, a wrong "
        "assumption you corrected, a file that behaves unexpectedly. Include "
        "file paths, line numbers and the exact pattern. Date it.\n\n"
        "Durable conclusions about the project itself belong in docs/ instead "
        "(see the `contrato` skill).\n\n"
        "If you genuinely learned nothing reusable, that is a fine answer - "
        "record nothing and move on. This check will not fire again for this "
        "task.\n\n"
        "IMPORTANT: reproduce your full final report in your next message, "
        "with any note about memory appended to the END of it. Only your last "
        "message reaches the coordinator, so a short reply here silently "
        "DESTROYS your findings - that failure has already happened twice.\n"
    )
    return 2


if __name__ == "__main__":
    sys.exit(main())
