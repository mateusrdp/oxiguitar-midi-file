# Contributing to oxiguitar-midi-file

Thank you for helping. Pull requests are welcome from anyone; the maintainer,
Mateus Rocha de Paula ([@mateusrdp](https://github.com/mateusrdp)), reviews them.
This repository is part of OxiGuitar and follows
[its contributing guide](https://github.com/mateusrdp/oxiguitar/blob/develop/CONTRIBUTING.md)
(code standards, file headers, test files, design principles). What follows
is the part every pull request meets first.

## Branches and pull requests

- `develop` is where work lands. Branch from it and open your pull request
  against it.
- `main` holds released work. Only the maintainer merges into it and tags
  releases.
- Feature branches are squash-merged into `develop`; `develop` is merged
  into `main` with a merge commit.
- CI must be green. The `ci-success` check sums up every other job.

## Tests for every feature

Every feature has tests, and a row on the manual's Features page (`docs/manual/src/features.md`) naming them. A pull request that adds or changes a feature, or a plugin, adds or updates both. Before opening it, run `python3 scripts/test-all.py --corpus DIR`: everything must pass. CI checks the Features page on every system it tests; the full run, on your own corpus and the newest OxiGuitar release, is yours, since the corpus is never uploaded.

## Sign-off (Developer Certificate of Origin)

We use the [Developer Certificate of Origin](https://developercertificate.org/)
instead of a contributor license agreement. By signing off a commit you state
that you wrote it, or otherwise have the right to submit it under the
project's license. Sign off with `-s`:

```sh
git commit -s -m "Describe the change"
```

This adds a line such as `Signed-off-by: Your Name <you@example.com>`, which
must match the commit's author email. To fix a branch afterwards:
`git rebase --signoff develop`, then force-push your branch.

Commits made by the Claude GitHub app, which writes much of the code with the
maintainer, carry a `Co-Authored-By: Claude` trailer instead. The DCO is a
statement by a person, so the check (`scripts/check-dco.sh`) skips them, and
also skips GitHub's squash merges of those pull requests into `develop` and
Dependabot's commits. The maintainer's review and merge of the pull request
into `main` is the human sign-off for that work.

You keep the copyright on what you contribute.
