# Git Branch Sync & Workflow Guide

This document explains how to manage branches in this repository when working with **protected `main` branch** (no direct pushes to main — use PRs).

Topics:
- Keep feature, fix and all other branches up-to-date with `main`.
- Avoid merge conflicts.
- Maintain a clean and consistent Git history.

---

## Overview

VES monorepo approach uses a protected `main` branch**.
All new work must happen in **feature, improvement or fix branches**, with updates flowing to `main` **via Pull Requests (PRs)** only.

```text
main  <- PR from feature/ branch
      <- PR from improvement/ branch
      <- PR from other branches...
```

After each PR merge, `main` moves forward. Each developer must sync their local branches with the lates remote `main` before continuing work.

---

## Basic Rules

1. Never push directly to `main`.
2. Always update your local `main` branch before branching or syncing.
3. Always push after merging or rebasing `main` into your branch.

---

## Standard Git Flow

1. Create a new branch from `main`

```console
git checkout main
git pull origin main
git checkout -b <branch_type/description_name>
```

Make your changes, commit and push

```console
git add .
git commit -m "Well made description based on What, Where, Why, How"
git push -u origin <branch_type/descriptive_name>
```

2. Keep your branch update with `main`

When new commits land in remote `main` (via merged PRs), sync your branch as follows:

Step 1 — Update local `main` branch
```console
git checkout main
git pull origin main
```

Step 2 — Merge or rebase update local `main` branch into your branch you're working on
Option A: Merge (safe, preserves history)
```console
git checkout <branch_type/description_name>
git merge main
```

Option B: Rebase (Clean, linear history)
```console
git checkout <branch_type/description_name>
git rebase main
```

Fix any conflicts if they appear, then continue:
```console
git add .
git commit                  # for merge
# or
git rebase --continue       # for rebase
```

Step 3 — Push updated branch
```console
git push                    # merge
# or
git push --force            # rebase
```

> Why push again?
> - "`merge` and `rebase` happen locally."
> - "`push` updates GitHub so the PR reflects the latest `main`."

---

## Merge PR into `main`

Once your branch is up-to-date and reviewed:
- Merge it via GitHub PR.
- Delete the branch if done.

Then, pull the updated remote `main` branch again locally before starting new work:
```console
git checkout main
git pull origin main
```

---

## Automation bash script

```bash
#!/bin/bash
# sync-branches.sh
git fetch origin
git checkout main
git pull origin main

for branch in feature/ improvement/performance fix/errors; do
    git checkout $branch
    git merge main
    git push
done

git checkout main
```

---
