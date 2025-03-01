# Changelog

## [0.4.0] - 2025-03-01

### Added
- `sync` subcommand, which performs git pull (fast-forward) and push (#21)
- Feature `vendored-openssl` to statically link openssl and libgit2 (#22)

### Fixed
- Git local config is now looked up. (#20)
- Git global config will not be polluted in test by default. (#20)

## [0.3.0] - 2024-12-02

### Added
- Add `status` subcommand to see storage and backup on given path or current working directory ([#17](https://github.com/qwjyh/xdbm/pull/17)).

### Changed
- Colored output for `storage list` and `backup list` ([#15](https://github.com/qwjyh/xdbm/pull/15))
- **BREAKING** Relative path is changed from `PathBuf` to `Vector<String>` for portability. This means that existing config files need to be changed.

## [0.2.1] - 2024-06-19

### Changed
- Dependencies are updated.
- Format of storage size printing has been changed due to the update of byte-unit.

### Fixed
- `libgit2-sys` was updated due to the security issue.

## [0.2.0] - 2024-05-21

### Changed
- Added CI on GitHub Actions (#10).
- Replaced `HashMap` with `BTreeMap` to produce cleaner diff (#11).

## [0.1.0] - 2024-03-18

### Added
- initial release
- `init` subcommand
- `storage add` subcommand
- `storage list` subcommand
- `storage bind` subcommand
- `path` subcommand
- `check` subcommand
- `backup add` subcommand
- `backup list` subcommand
- `backup done` subcommand
- `completion` subcommand

[0.4.0]: https://github.com/qwjyh/xdbm/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/qwjyh/xdbm/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/qwjyh/xdbm/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/qwjyh/xdbm/releases/tag/v0.2.0
[0.1.0]: https://github.com/qwjyh/xdbm/releases/tag/v0.1.0
