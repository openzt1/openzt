# zthabitat-related-functions-plan - stage 7 agent fallout

## Status: FALLOUT (plan phase) - 2026-09-27T08:02:22+00:00

**Stage:** 7 - Per-Neighbor Show Portal Lookup (`ZTHabitat::getShowPortal`)
**Branch:** `feature/reimplement-zthabitatmgr`

## No agent explanation available
The session ended without calling `request_human_intervention`, `mark_stage_complete`, or (in the plan phase) `ExitPlanMode` - outcome recorded as `no_signal`. This can mean the phase timed out, the session crashed, or it stopped for a reason the transcript doesn't make explicit.

## Best-effort transcript summary
I'll explore the repo to understand the current state relevant to this stage.
Now let me look at the habitat.rs structure and support.rs helpers.
Ghidra MCP is live. Let me pull the real decompile of `getShowPortal` and explore the surrounding code in parallel.
The Windows decompile confirms the structure. Let me get the actual disassembly to resolve the exact branch semantics, and check the generated.rs entries for `tree24::insert` and `AI_cls_0x404fd6::find`.
The disassembly reveals the tank branch may not actually insert on miss — the polarity hinges on what `0x004031b1` (the `find`) computes and what's at the `0x005ab33a` tail. Let me verify both.
`0x004031b1` is an exact-match `find` (lower_bound + equality confirm) — so the tank branch's insert tail is **dead code** and the function is a pure read. This contradicts the plan sketch. Let me corroborate the remaining field semantics: what `+0xc` is, and the `tree24` handle shape at `+0x20`.
Two key findings: `field_0xc` is confirmed as the amphibious set's `_Mysize` (the generic insert increments `handle+4`), and there's an **existing `GET_SHOW_PORTAL.original()` call site** at `zthabitatmgr.rs:3075` that becomes a re-entry site once detoured. Let me examine that call site and the cold-split recursion body.
The cold-split confirms the recursion shape exactly. Now let me identify which function contains the `GET_SHOW_PORTAL.original()` call site, and find the battery registration list.
Prompt is too long
[result] subtype=success is_error=True stop_reason=stop_sequence

## Resuming
After resolving the above, re-file this stage explicitly - it will not be picked up automatically:

```
openzt-orchestrator resume zthabitat-related-functions-plan --stage 7 --as not_started
```
