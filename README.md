# xdbm
_Cross device backup manager_,
which manages backups on several storages mounted on multiple devices with a single repository.

## Usage
1. `xdbm init` to setup new device(i.e. PC).
2. `xdbm storage add` to add storages, or `xdbm storage bind` to make existing storages available on new device.
3. `xdbm backup add` to add new backup configuration.
4. `xdbm backup done` to tell xdbm to write backup execution datetime.
5. `xdbm storage list` and `xdbm backup list` to see their status.

### Hints
- `xdbm path` to see the path of repository. Use `git` to sync it among the devices.
- `xdbm completion <SHELL>` to generate completion scripts for shells.
- Give `--help` to see helps on the commands.

## TODO:
- [x] split subcommands to functions
- [x] write test for init subcommand
  - [x] write test with existing repo
  - [x] with ssh credential
    - [x] ssh-agent
    - [x] specify key
- [ ] write test for storage subcommand
  - [x] storage add online
  - [x] storage add directory
  - [ ] storage list
- [x] update storage bind command
- [ ] add storage remove command
- [ ] add sync subcommand
- [x] add check subcommand
  - [x] check that all parents exist
- [x] reorganize cmd option for storage
  - [x] use subcommand
- [x] backup subcommands
  - [x] backup add
    - [x] test for backup add
  - [x] backup list
    - [x] status printing
  - [x] backup done
- [x] fancy display
- [ ] json output
- [ ] no commit option

<!-- vim: set sw=2 ts=2:  -->
