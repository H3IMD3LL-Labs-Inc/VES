# Overview
Since working on code and any other contributions requiring PRs will need you to fork the repo then clone it locally, it is recommended you follow this branching strategy to make both your time and the maintainers time easier.

> P.S. This project has only one maintainer at the time of writing so try and make their time alittle bit easier ;)

## Core Repo Branches
1. Main
   - This is branch is always deployable, with the latest code changes. Always create the branch you intend to use to make changes from here.
   - DO NOT create PR from your forked repo branch into main, these will be automatically rejected and not considered.
   - [BUILDING.md](BUILDING.md) assumes you are using this branch to build the binary.
   - This branch is long-lived and permanent.
2. Integration
   - This branch contains all PRs from all forked repo branches, DO NOT create checkout the branch you intend to use to make changes from here.
   - This branch is where all change PRs should target when running `git push`.
   - The repo's test suite is run from here, passing these is the only way to ensure your PRs make it into `main` branch. ALWAYS MAKE SURE your contributions' PRs from your fork target this branch.
   - This branch is long-lived and permanent.
   
This is basically how the repo's branches are structured, keep this in mind when you fork the repo to start contributing.

## Your Repo Fork Branches
- Now, with the repo forked and cloned locally to your laptop you can start building PRs, as many meaningful ones as you want. But you should keep in mind how you create your "change branches"(These are the branches you create to make changes to the source).

1. Always follow this pattern when creating your "change branches"
```bash
git checkout -b <contribution-type>/<name-of-work>
```

2. Always ensure any changes you make are withing the change branch you created above.

3. Commit each incremental, logical change you make. Avoid "bulk committing" multiple unrelated changes at once. Use Git's patch mode to selectively stage only the relevant parts of your changes. COMMITS THAT BUNDLE unrelated changes may be asked to be split during review.
```bash
git add -p <file-affected> \n 
```

4. Push the changes you've made to your forked repo, and create a PR targeting `integration` branch on the source repo. And listen for feedback from the maintainer, feedback should have a delay of between 10 minutes to 24 hours at the latest, depending on the PR. You could work on other contributions in the mean time, don't let this bog you down from contributing.

5. If your done with the specific change branch you were working on, ensure it is deleted and you've created a new change branch for a different change you're making before you start working on it.

## A Good Rule Of Thumb
- DO NOT TARGET PRs on your forked repo onto the `main` branch of the source repo, you'll make your contributor experience terrible.
- You DO NOT NEED to go through the integration branch in your forked repo, this will just add an extra step to your workflow and have you doing something that is already handled in the source repo, basically doing work that is of zero benefit to yourself and the project.
- This project has only one maintainer **at the time of writing this doc** so alot of workflows in the source repo are automated and manual reviews and instructions can take between 10 minutes to 24 hours, please be patient, it's not that your PR might be rejected with no comment, but rather the maintainer is helping another contributor. This is why the doc was made, to help both you and the maintainer have a smoother contributor experience.
