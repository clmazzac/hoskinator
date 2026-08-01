#!/usr/bin/env bash
# Blocks `gt submit` until /style-review has run against the exact commit being submitted.
# The marker records the reviewed commit SHA; amending invalidates it.

set -uo pipefail

# The `if` filter in settings.json is a substring pre-filter, so it also fires on commands that
# merely mention the string. Re-check precisely: only a segment that actually starts with
# `gt submit` counts.
command=$(node -e '
  let s = "";
  process.stdin.on("data", d => s += d).on("end", () => {
    let cmd = "";
    try { cmd = JSON.parse(s).tool_input?.command ?? ""; } catch {}
    const submits = cmd
      .split(/[;\n]|&&|\|\|/)
      .some(seg => /^\s*gt\s+submit\b/.test(seg));
    process.stdout.write(submits ? "yes" : "no");
  });
' 2>/dev/null) || exit 0

[ "$command" = "yes" ] || exit 0

# Any failure here means we cannot tell whether a review happened. Allow rather than wedge the
# user out of submitting; this is a guardrail, not a security control.
repo_root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0
head=$(git rev-parse HEAD 2>/dev/null) || exit 0

marker="$repo_root/.git/style-review-marker"
reviewed=$(cat "$marker" 2>/dev/null || true)

if [ "$reviewed" = "$head" ]; then
  exit 0
fi

if [ -z "$reviewed" ]; then
  detail="no review has been recorded"
else
  detail="the recorded review is for ${reviewed:0:12}, but HEAD is now ${head:0:12} — the commit changed after it was reviewed"
fi

reason="Blocked: $detail. Run the /style-review skill, which reads docs/STYLE.md, reviews the diff line by line, applies fixes, and records the reviewed commit. Record the marker only after the final amend, or it will be stale again."

# Emitted without jq, which is not installed everywhere. The reason is single-line by
# construction; backslashes and quotes are escaped so it stays valid JSON regardless.
escaped=$(printf '%s' "$reason" | sed 's/\\/\\\\/g; s/"/\\"/g')

printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"%s"}}\n' "$escaped"
