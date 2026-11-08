# Windows workspace hygiene and controlled cleanup (2026-11-08)

**Goal:** Stop the growth of local Optic Rift test worktrees, copied model weights, temporary builds, QA profiles and screenshots without deleting unique work, private content, runtime caches owned by the main account, or evidence that has not been uploaded.

The repository previously carried 17 open PRs. It now has 7 distinct, independently reviewable source PRs, with the ten superseded discussions preserved and no forced merge. **Local disk cleanup is separate from GitHub PR closure.** This document authorizes neither Qortara bypass nor deletion of the user's personal files.

## Rules for all development agents

- **No new PR for routine fixes.** Amend the appropriate existing review owner (#1, #13, #14, #15, #28, #34, #35), using ordinary pushes, not force pushes and `[skip ci]` when requested. Separate branches may be used temporarily for local QA only; list and retire them after receipts are attached.
- The canonical **Photo/Video Studio implementation review** is #35. **All crop changes are on this PR**, not a new PR.
- Never delete a worktree just because the related PR was administratively closed. First confirm every unique file/commit is incorporated in the consolidated head and any qualification evidence is posted/archived.
- Do not delete in-progress worktrees, dirty tracked/untracked states, locked worktrees, protected/default branches, unrelated repositories, or user-owned models/media.
- Do not use `git clean -fdx`, `git worktree remove --force`, `git branch -D`, `Remove-Item -Recurse -Force` on an unapproved directory, or automatic cache cleanup targeting the owner's normal application profile.
- Do not assume a OneDrive-synced Pictures location is local/private. Synthetic exports in it are **user-reviewed cleanup only**.

## Phase 1: Inventory before cleanup (no writes)

Run from an isolated Windows PowerShell terminal, in the actual Git checkout:

```powershell
git status --short
git rev-parse --show-toplevel
git worktree list --porcelain
git branch -vv
.\scripts\audit_qa_worktrees.ps1 -Repository .
git worktree prune --dry-run --verbose
```

Capture output in a dated plain-text receipt **outside** the repository worktrees, with a summary of drive free space before/after. For each worktree record full path, branch, head SHA, PR number, dirty state and whether the head's unique changes are present in #35 or another surviving PR.

Inventory Optic Rift QA profiles (e.g. `%LOCALAPPDATA%\studio.mythologiq.opticrift.qa`), model/runtime copies, dev `node_modules`, Rust `target` directories, test screenshots, staging exports and OneDrive Pictures test patterns. Record approximate sizes, whether they belong to the owner's real profile versus an isolated synthetic test profile, retention requirement and observed file hashes. Do not delete yet.

## Phase 2: Safe worktree removal

Only remove a worktree that is both **clean and independently verified redundant** with a nominated surviving PR, and only when the actual complete path is explicitly supplied:

```powershell
.\scripts\audit_qa_worktrees.ps1 -Repository . -Apply -RemoveWorktrees 'C:\absolute\path\to\old-qa-worktree' -WhatIf
.\scripts\audit_qa_worktrees.ps1 -Repository . -Apply -RemoveWorktrees 'C:\absolute\path\to\old-qa-worktree'
```

The second command requires a user confirmation; it calls `git worktree remove` without `--force`, and refuses dirty/locked/primary worktrees. **Review the actual candidates and their output before repeating across other paths.** Stale administrative refs may be pruned after a separate `--dry-run` inspection; do not silently delete branches or tags. Prefer `git branch -d` only after actual safe ancestry/merge confirmation. An administratively closed but **unmerged** PR does not qualify as a merged branch.

## Phase 3: Separate model/cache/test cleanup

For a sandbox/QA profile only, do not delete models or runtimes just because they are large. Preserve unique downloaded weights, license/provenance records and their hashes, as well as anything still needed for an active exact-head Windows qualification. If a verified byte-identical copy of the 3.2GB runtime and 4.2GB SSD-1B model was staged into a **named disposable QA profile**, and evidence no longer needs it, the owner may approve the explicit QA-profile path for deletion. Never remove the real `%LOCALAPPDATA%\studio.mythologiq.opticrift` profile or the original source weights.

For `node_modules` and `target`, prefer deleting only confirmed unused build outputs in an already-retired sandbox, after checking for locks/running processes. Do not purge caches belonging to other repositories. Evidence branches and screenshots must retain at least one discoverable, verified authoritative copy. OneDrive exports require separate user approval and confirmation of which copies are safe to remove.

## Evidence and completion

Post a single hygiene receipt on [issue #2](https://github.com/Knapp-Kevin/optic-rift/issues/2) or the canonical development tracker with:
- initial/final worktree count, exact paths retired and why each was safe;
- number of untracked/dirty worktrees explicitly retained;
- initial/final disk free space and approximate GB reclaimed by category;
- original vs copy model hashes, which files were *not* deleted, and all approvals for deletions;
- branch/PR state readback, and next QA worktree naming/reuse convention.

A cleanup is **not complete** merely because `git worktree prune` ran or the PR count dropped. Full user-owned image/video Studio remains on open issue #29; native trim/export remains on issue #36.
