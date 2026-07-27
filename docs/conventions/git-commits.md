# Git Commits

## Commit Format

Commit messages MUST follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

The following types are RECOMMENDED:

- `init`: Establishment of a repository or crate
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `refactor`: Code restructuring without behavioral changes
- `build`: Build system changes (Cargo, Nix, etc.)
- `chore`: Dependency updates, version bumps, and miscellaneous tasks
- `ci`: CI/CD automation configuration changes

Scope is RECOMMENDED and is usually specified by crate:

- Sub-crates use their crate names: `selector4nix-core`, `selector4nix-actor`, etc.
- Sub-crates for system tests use their paths: `tests/cache-persistence`, etc.
- `nix` for changes to Nix-related code (`flake.nix`, `nix/`, etc.).
- `deps` for dependency changes, which is typically used with `chore`
- When a commit spans multiple crates, the scope MAY be `*`, a comma-separated list, or a glob pattern for compactness.

The header MUST describe what the commit does. The body is OPTIONAL.

## Commit Granularity

Commits SHOULD reflect the implementation approach and reduce reviewer burden.

Giant commits SHOULD NOT be created. The additions and deletions of a single commit SHOULD be kept around 300 lines, unless a large-scope refactor makes it unavoidable to change a significant amount of code at once.

Each commit SHOULD be functionally cohesive. For example, a commit MAY represent a complete vertical slice of a feature, a partial implementation within a single layer, or changes confined to a single crate. In general, each commit SHOULD address only one concern, with the granularity adjusted according to the number of changed lines.

On feature branches, each commit SHOULD pass compilation, tests, formatting, and lint checks, unless the change scope is too large to make this practical.

On the main branch, each commit MUST pass all checks, following the [Not Rocket Science Rule](https://graydon2.dreamwidth.org/1597.html).

## Branching Strategy

This project uses trunk-based development. Each feature, bug fix, refactor, or other change MUST be developed on a separate branch and merged directly into the main branch.

Branch names SHOULD follow a format similar to Conventional Commits: `feat-do-something`, `fix-some-bug`, etc. There is no strict limit on the number of words, but 4 to 5 is generally acceptable. Branch names SHOULD NOT be excessively long.

When merging a branch into the main branch, a merge commit MUST be used. Rebase merges MUST NOT be used, because they expose fine-grained intermediate commits to the main branch history, violating the Not Rocket Science Rule and making bisect operations unreliable. Squash merges MUST NOT be used, because they discard all intermediate history and the resulting commit message cannot reflect the implementation process.

## Commit History

On feature branches, it is RECOMMENDED to rewrite commit history. The history does not need to strictly reflect the actual development process, but SHOULD make review and future retrospection of the implementation approach easier. Commits MAY be merged or split within reasonable bounds.

The following patterns are RECOMMENDED:

- If a feature implementation commit has a defect that is fixed in a later commit, the fix SHOULD be fixup'd into the corresponding feature implementation commit.
- Different files SHOULD be split into separate commits according to the commit granularity requirements.
- Reviewer feedback SHOULD be applied directly to the corresponding commits rather than appended as new fixup commits. Force-pushing to PR branches is permitted for this purpose.
