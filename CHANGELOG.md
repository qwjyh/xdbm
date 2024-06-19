# Changelog

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

[0.2.1]: https://github.com/qwjyh/xdbm/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/qwjyh/xdbm/releases/tag/v0.2.0
[0.1.0]: https://github.com/qwjyh/xdbm/releases/tag/v0.1.0
