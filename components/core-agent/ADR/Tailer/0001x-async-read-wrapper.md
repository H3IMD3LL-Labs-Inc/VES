# ADR: Custom future-lifecycle-aware `AsyncRead` Wrapper for Graceful Shutdown

## Status: *Accepted*

## Context
VES' source is organized into multiple independent subsystems, along with the associated documentation and ADRs. There's a clear need for;
1. Clear code ownership by subsystem.
2. Work isolation where each subsystem's development in the core-agent does not disrupt other parts
3. Maintainable and predictable release pipeline, ensuring `main` is always deployable and healthy.
4. GitHub Actions workflows that verify correctness of both individual subsystems and the integrated system.
5. Avoiding the usual common pitfalls that come with long-lived branches, including branch-drift, integration conflicts, review difficulties, and CI complexity.

The usual approach common in most OSS projects hosted on platforms such as GitHub, rely on very short-lived feature branches and frequent merges into `main`. This minimizes branch drift and simplifies CI, it does not provide natural ownership isolation for large subsystems and can make coordinating changes that affect multiple subsystems more error-prone.

Therefore, an approach where a long-lived permanent branch per subsystem is considered. This approach is supported by another permanently lived integration branch, disciplined PR workflows, and automated CI/CD. This design seeks to retain subsystem isolation while mitigating the typical downsides of long lived permanent branches.

## Decision
A permanent long-lived branch per subsystem model will be adopted with the following rules and structures;

1. Branching Model
- Permanent branches:
  - One long-lived branch per subsystem (e.g., tailer-subsystem, watcher-subsystem, etc.).
  - Dedicated branches for docs and ADRs (e.g., adr-updates, docs-updates).
  
- Your work branches:
  - Always branch off from the relevant permanent branch.
  - Are short-lived, ideally lasting only hours to days and deleted after their merged into their relevant permanent branch.
  - PRs here target their specific permanent branch, never the `main` branch.
  
2. Integration Branch
- A dedicated `integration` branch will be used to periodically merge all permanent branches.
- GitHub Actions will run the full system test suite on this branch to confirm nothing is broken.
- ONLY CHANGES THAT PASS INTEGRATION testing are merged into `main`
- Optional tagging or labelling for integration merges to track exactly which subsystem commits are combined.
- Integration merges can be automated using GitHub Actions, scheduled at regular intervals or triggered manually before major releases.

3. Synchronization With Main
- The permanent branches will regularly sync with `main`, based on how the integration branch automation is set up, e.g., daily or weekly.
- This reduces drift, ensures conflicts are resolved early, and keeps branches up to date with hotfixes or shared changes in `main`.
- `main` and the permanent branches syncing can be automated via a GitHub Actions scheduled workflow.

4. CI/CD
- Permanent branch CI:
  - Runs tests relevant to the subsystem the permanent branch holds.
  - Ensure any changes are correct, locally to the subsystem's code.

- Integration branch CI:
  - Runs the full system test suite combining all subsystems, ensuring all changes in different permanent branches are correct.
  - Detects cross-subsystem integration conflicts early.
  
- Main CI/CD:
  - Only receives changes from the tested and passed integration branch.
  - Ensures `main` is always deployable.
  - CI checks and branch protection rules enforce that merges to `main` occur only after successful and passed integration testing.
  
- Testing and merging integration branch to `main` should be automated to occur at a set interval in a GitHub Actions workflow.

5. Pull Request Policy
- PRs must always target the permanent branch corresponding to the subsystem the change belongs to.
- PRs must pass CI and receive code review before merging.
- Your work branches are deleted promptly after merging to avoid too much clutter.
- Documentation and ADR changes are treated similarly, with dedicated branches for isolation and manual review.

6. Mitigating Branch Proliferation
- Only one permanent branch per subsystem or per purpose(e.g., docs, ADR).
- Contributions are made using short-lived work branches checked out from the latest version of the permanent branch that corresponds to what you're working on.
- Immediately after merging your working branch to the corresponding permanent branch,
the work branch is deleted.
- Avoid growing the permanent branches to an unmanageable number.

7. Other Stuff To Consider
- Feature Flags for partially completed features/code changes; this will allow merging experimental/incomplete changes into their corresponding permanent branches without breaking integration, this would have come in handy for PR [#87](https://github.com/H3IMD3LL-Labs-Inc/VES/pull/87).

- Automate notifications on branch drift, CI failures, or pending integration merges.

- Keep contribution process rapid despite the strictness required for an automated contributor contributor, possible automations;
  - Syncing latest `main` to each permanent branch, atleast daily(TBD).
  - Integrating tested and passed changes from permanent branches to `main`, also atleast daily.
  - Ensure all GitHub Actions are performant and fast, to lower time spent in CI/CD workflows for tested and passed PRs.

## Consequences (but mitigated by "Other Stuff To Consider")
- Subsystem isolation maintained
- Main branch is always deployable
- Reduced permanent branch drift
- Integration friction minimized
- CI/CD workflow clear and automated 
- Review and PR process clarity
- Branch proliferation is controlled
- Requires discipline from contributors and maintainer
- Requires more automation to be set up and maintained
- Slightly slower integration process
- Potential for merge conflicts during integration
