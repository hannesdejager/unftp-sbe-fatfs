# AGENTS.md

## Repository overview

`unftp-sbe-fatfs` currently provides read-only FAT image storage for libunftp.
The backend interface comes from `unftp-core`; `libunftp` is used by examples.
The implementation is in `src/lib.rs`, with a sample image and server in `examples/`.

## Development

- The backend currently supports read-only access. Write support is a valid future feature;
  when adding it, test each operation, update the README and API docs, and review the release impact.
- FAT image reads currently use synchronous file I/O. Do not describe them as nonblocking.
- Keep public API documentation and README examples accurate and compilable.
- Run `cargo fmt --all`, the relevant tests, and `make pr-prep` before submitting changes.

## Release conventions

- Run `cargo upgrade -v`, review incompatible updates separately, and check the resulting lockfile.
- Review changes since the last published version before choosing a version. For `0.y.z` with
  `y > 0`, increment `z` for compatible changes and `y` for breaking changes.
- Update the package version, dependency examples, rustdoc links, and release notes together.
- Run `make pr-prep` and `cargo publish --dry-run` before publishing.
- Build every example with `cargo build --examples`. Run `cargo run --example basic` from the
  repository root against `examples/my.img`, then connect with an FTP client and verify directory
  listing and a file download. Compilation alone does not verify runtime behavior.
- Use a release commit naming the crate and version. Use `v{version}` for release tags unless
  the repository establishes another convention.
- Do not publish, tag, push, or create a GitHub release without the user's explicit request.

## Git commit messages

- Do not use conventional commits.
- Separate the subject from the body with a blank line.
- Keep the subject around 50 characters, capitalize it, use the imperative mood, and omit
  the final period.
- Wrap the body at 72 characters. Explain what changed and why for the next reader of `git log`.
