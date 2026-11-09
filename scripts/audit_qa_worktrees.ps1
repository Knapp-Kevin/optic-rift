#Requires -Version 5.1
<#
.SYNOPSIS
  Audits Optic Rift Git worktrees on Windows without deleting anything.
.DESCRIPTION
  Default is read-only. With -Apply and an explicit -RemoveWorktrees exact path
  list, only existing Git worktrees that are not the primary worktree and have
  zero tracked OR untracked changes are eligible for Git's safe worktree remove.
  Never uses --force, git clean, branch -D, or deletes standalone folders.

  Model files, QA profiles, screenshots, isolated runtimes, installer builds,
  caches and OneDrive exports are intentionally NOT deleted by this script.
.EXAMPLE
  .\scripts\audit_qa_worktrees.ps1
.EXAMPLE
  .\scripts\audit_qa_worktrees.ps1 -Apply -RemoveWorktrees 'C:\dev\optic-rift-pr31' -WhatIf
.EXAMPLE
  .\scripts\audit_qa_worktrees.ps1 -Apply -RemoveWorktrees 'C:\dev\optic-rift-pr31'
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [string]$Repository = '.',
    [switch]$Apply,
    [string[]]$RemoveWorktrees = @()
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = (& git -C $Repository rev-parse --show-toplevel 2>&1)
if ($LASTEXITCODE -ne 0 -or -not $root) { throw 'Repository is not a valid Git checkout.' }
$root = [IO.Path]::GetFullPath(([string]$root).Trim())
$lines = @(& git -C $root worktree list --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'Could not inspect Git worktrees.' }
$worktrees = [Collections.Generic.List[object]]::new()
$current = $null
foreach ($line in $lines) {
    if ($line.StartsWith('worktree ')) {
        if ($null -ne $current) { $worktrees.Add($current) }
        $current = [ordered]@{
            Path = [IO.Path]::GetFullPath($line.Substring(9))
            Head = ''
            Branch = ''
            Locked = $false
            Prunable = $false
        }
    } elseif ($null -ne $current -and $line.StartsWith('HEAD ')) {
        $current.Head = $line.Substring(5)
    } elseif ($null -ne $current -and $line.StartsWith('branch ')) {
        $current.Branch = $line.Substring(7)
    } elseif ($null -ne $current -and $line.StartsWith('locked')) {
        $current.Locked = $true
    } elseif ($null -ne $current -and $line.StartsWith('prunable')) {
        $current.Prunable = $true
    }
}
if ($null -ne $current) { $worktrees.Add($current) }
if ($worktrees.Count -eq 0) { throw 'Git did not list any worktrees.' }

$primary = $worktrees[0].Path
Write-Host "Optic Rift worktree audit: $root"
Write-Host "Discovered $($worktrees.Count) Git worktrees; primary: $primary"
$comparison = [StringComparison]::OrdinalIgnoreCase
$selected = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($path in $RemoveWorktrees) {
    if (-not [IO.Path]::IsPathRooted($path)) { throw "Removal path must be absolute: $path" }
    [void]$selected.Add([IO.Path]::GetFullPath($path))
}
if ($Apply -and $selected.Count -eq 0) { throw 'Apply requires at least one explicit -RemoveWorktrees path.' }
foreach ($requested in $selected) {
    if (-not ($worktrees | Where-Object { [string]::Equals($_.Path, $requested, $comparison) })) {
        throw "Requested path is not an actual worktree in this checkout: $requested"
    }
}

foreach ($wt in $worktrees) {
    $isPrimary = [string]::Equals($wt.Path, $primary, $comparison)
    $exists = Test-Path -LiteralPath $wt.Path -PathType Container
    $clean = $false
    if ($exists) {
        $status = @(& git -C $wt.Path status --porcelain=v1 --untracked-files=all)
        $clean = $LASTEXITCODE -eq 0 -and $status.Count -eq 0
    }
    $eligible = -not $isPrimary -and $exists -and $clean -and -not $wt.Locked -and -not $wt.Prunable
    $label = if ($isPrimary) { 'PRIMARY' } elseif (-not $exists) { 'MISSING: manually inspect/prune' } elseif ($wt.Locked) { 'LOCKED' } elseif (-not $clean) { 'DIRTY: retain' } else { 'CLEAN: eligible' }
    Write-Host ("[{0}] {1} | {2} | {3}" -f $label, $wt.Path, $wt.Branch, $wt.Head)
    if ($selected.Contains($wt.Path)) {
        if (-not $eligible) { throw "Refusing to remove ineligible worktree: $($wt.Path) ($label)" }
        if (-not $Apply) {
            Write-Host "  DRY RUN: pass -Apply to ask Git to remove this clean worktree."
        } elseif ($PSCmdlet.ShouldProcess($wt.Path, 'git worktree remove (no force)')) {
            & git -C $root worktree remove -- $wt.Path
            if ($LASTEXITCODE -ne 0) { throw "Git refused to remove $($wt.Path); no force was attempted." }
            Write-Host "  Removed Git worktree: $($wt.Path)"
        }
    }
}
Write-Host 'Other storage (model copies, QA profiles, screenshots, OneDrive exports): READ-ONLY audit separately.'
Write-Host 'No git prune, branch deletion, or model/cache cleanup was performed.'
