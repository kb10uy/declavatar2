# How to contribute

Thanks for contributing to declavatar2!

## Overview

### Did you found a bug?

- Feel free to open issues!
- English or Japanese are preferred.

### Would you like to implement new feature?

- Feel free to open PRs!

### Other

- You can put small (1~2 files) cosmetic change into another PR.
    - Please make independent commit, don't mix with other changes!

## Opening PRs

- Maintainers may directly modify the contents of a PR when necessary.

## Commit Guidance

- Avoid mixing refactors and behavior changes unless necessary.
- To make debugging and `git bisect` easier, each commit should at least build successfully.
    - If a change is too large to split into fully complete commits, mark progress clearly in the commit message (for example: `WIP 1/n`, `WIP 2/n`).
- Use imperative commit messages.
    - No strict format is required. ~~I don't like Conventional Commits ;)~~

## Testing Guidance

- Add unit tests for parser/transform behavior changes.
- For interop format changes, add or update a round-trip test, regenerate the golden blob, and bump the matching version in `assets/interop-format.md`.

## Security and Safety

- Be careful with FFI boundaries: validate pointers, lengths, and ownership rules.

--------

kb10uy
