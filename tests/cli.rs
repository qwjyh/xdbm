mod integrated_test {
    use std::{
        fs::{self, DirBuilder, File},
        io::{self, BufWriter, Write},
        path,
    };

    use anyhow::{Context, Ok, Result, anyhow};
    use assert_cmd::{Command, assert::OutputAssertExt};
    use git2::Repository;
    use log::{debug, trace};
    use predicates::{boolean::PredicateBooleanExt, prelude::predicate};

    const IS_GIT_CONFIG_WRITABLE: &str = "XDBM_ENABLE_OVERWRITE_GITCONFIG";

    /// Setup global gitconfig if it doesn't exist.
    ///
    /// # Errors
    ///
    /// This function will return an error if it failed to get git global config and environment
    /// variable [XDBM_ENABLE_OVERWRITE_GITCONFIG](`IS_GIT_CONFIG_WRITABLE`) is not set.
    fn setup_gitconfig() -> Result<()> {
        let config = git2::Config::open_default().expect("failed to get default");
        if config.get_string("user.name").is_ok() && config.get_string("user.email").is_ok() {
            return Ok(());
        };

        match std::env::var_os(IS_GIT_CONFIG_WRITABLE) {
            Some(_) => {
                debug!(
                    "global git config not found & env var `{}` found",
                    IS_GIT_CONFIG_WRITABLE
                );
            }
            None => {
                eprintln!("Failed to get git global config");
                eprintln!(
                    "Set env var `{}` to set automatically (mainly for CI)",
                    IS_GIT_CONFIG_WRITABLE
                );
                return Err(anyhow!("failed to get git global config"));
            }
        };

        let config_file = git2::Config::find_global().map_or_else(
            |e| {
                trace!("global git config file not found: {e:?}");
                Ok(dirs::home_dir()
                    .context("Failed to get home dir")?
                    .join(".gitconfig"))
            },
            Ok,
        )?;
        let f = match File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(config_file)
        {
            io::Result::Ok(f) => f,
            io::Result::Err(_err) => return Ok(()),
        };
        let mut buf = BufWriter::new(f);
        buf.write_all(
            r#"
[user]
        email = "test@example.com"
        name = "testuser"
"#
            .as_bytes(),
        )?;

        Ok(())
    }

    fn run_sync_cmd(config_dir: &path::Path, use_cl: bool) -> Result<()> {
        if use_cl {
            Command::cargo_bin("xdbm")?
                .arg("-c")
                .arg(config_dir)
                .args(["sync", "-vvvv"])
                .assert()
                .success();
        } else {
            Command::cargo_bin("xdbm")?
                .arg("-c")
                .arg(config_dir)
                .args(["sync", "-vvvv", "-u"])
                .assert()
                .success();
        }
        Ok(())
    }

    #[test]
    fn single_device() -> Result<()> {
        let config_dir = assert_fs::TempDir::new()?;
        setup_gitconfig()?;
        // init
        let mut cmd = Command::cargo_bin("xdbm")?;
        cmd.arg("-c")
            .arg(config_dir.path())
            .arg("init")
            .arg("testdev")
            .arg("-vvvv");
        cmd.assert().success().stdout(predicate::str::contains(""));
        eprintln!("{:?}", fs::read_dir(config_dir.path())?.collect::<Vec<_>>());
        assert_eq!(
            std::fs::read_to_string(config_dir.path().join("devname"))?,
            "testdev\n"
        );

        // storage add
        let storage = assert_fs::TempDir::new()?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir.path())
            .arg("storage")
            .arg("add")
            .arg("online")
            .arg("--provider")
            .arg("sample_provider")
            .arg("--capacity")
            .arg("1000000000000")
            .arg("--alias")
            .arg("alias")
            .arg("online_storage")
            .arg(storage.path())
            .assert()
            .success();

        // storage list
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir.path())
            .arg("storage")
            .arg("list")
            .assert()
            .success()
            .stdout(predicate::str::contains("online_storage"));

        // backup add
        let target_from = storage.join("foo/bar");
        let target_to = storage.join("aaa/bbb/ccc");
        DirBuilder::new()
            .recursive(true)
            .create(target_from.clone())?;
        DirBuilder::new()
            .recursive(true)
            .create(target_to.clone())?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir.path())
            .arg("backup")
            .arg("add")
            .arg("--src")
            .arg(target_from)
            .arg("--dest")
            .arg(target_to)
            .arg("sample_backup")
            .arg("external")
            .arg("rsync")
            .arg("with some note")
            .assert()
            .success();

        // backup list
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir.path())
            .arg("backup")
            .arg("list")
            .assert()
            .success()
            .stdout(predicate::str::contains("sample_backup"));

        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir.path())
            .arg("backup")
            .arg("done")
            .arg("sample_backup")
            .arg("0")
            .assert()
            .success();

        Ok(())
    }

    #[test]
    fn two_devices_with_same_name() -> Result<()> {
        // 1st device
        let config_dir_1 = assert_fs::TempDir::new()?;
        setup_gitconfig()?;
        let mut cmd1 = Command::cargo_bin("xdbm")?;
        cmd1.arg("-c")
            .arg(config_dir_1.path())
            .arg("init")
            .arg("first");
        cmd1.assert().success().stdout(predicate::str::contains(""));

        // bare-repo
        let bare_repo_dir = assert_fs::TempDir::new()?;
        let _bare_repo = Repository::init_bare(&bare_repo_dir)?;
        // push to bare repository
        let repo_1 = Repository::open(&config_dir_1)?;
        let upstream_name = "remote";
        let mut repo_1_remote =
            repo_1.remote(upstream_name, bare_repo_dir.path().to_str().unwrap())?;
        repo_1_remote.push(&[repo_1.head().unwrap().name().unwrap()], None)?;
        trace!("bare repo {:?}", bare_repo_dir.display());
        println!("{:?}", bare_repo_dir.read_dir()?);
        // set up upstream branch
        let (mut repo_1_branch, _branch_type) = repo_1.branches(None)?.next().unwrap()?;
        println!("head {}", repo_1.head().unwrap().name().unwrap());
        repo_1_branch.set_upstream(Some(
            format!(
                "{}/{}",
                upstream_name,
                repo_1_branch.name().unwrap().unwrap()
            )
            .as_str(),
        ))?;

        // 2nd device
        let config_dir_2 = assert_fs::TempDir::new()?;
        setup_gitconfig()?;
        let mut cmd2 = Command::cargo_bin("xdbm")?;
        cmd2.arg("-c")
            .arg(config_dir_2.path())
            .arg("init")
            .arg("first")
            .arg("-r")
            .arg(bare_repo_dir.path());
        cmd2.assert().failure();
        Ok(())
    }

    #[test]
    fn directory_without_parent() -> Result<()> {
        // 1st device
        let config_dir_1 = assert_fs::TempDir::new()?;
        setup_gitconfig()?;
        let mut cmd1 = Command::cargo_bin("xdbm")?;
        cmd1.arg("-c")
            .arg(config_dir_1.path())
            .arg("init")
            .arg("first");
        cmd1.assert().success().stdout(predicate::str::contains(""));

        // add storage
        let sample_storage = assert_fs::TempDir::new()?;
        let mut cmd_add_storage = Command::cargo_bin("xdbm")?;
        cmd_add_storage
            .arg("-c")
            .arg(config_dir_1.path())
            .arg("storage")
            .arg("add")
            .arg("directory")
            .arg("--alias")
            .arg("gdrive")
            .arg("gdrive")
            .arg(sample_storage.path());
        cmd_add_storage
            .assert()
            .failure()
            .stderr(predicate::str::contains("No storages found"));

        Ok(())
    }

    #[test]
    fn two_devices() -> Result<()> {
        // 1st device
        //
        // devices: first
        let config_dir_1 = assert_fs::TempDir::new()?;
        setup_gitconfig()?;
        let mut cmd1 = Command::cargo_bin("xdbm")?;
        cmd1.arg("-c")
            .arg(config_dir_1.path())
            .arg("init")
            .arg("first");
        cmd1.assert().success().stdout(predicate::str::contains(""));

        // bare-repo
        let bare_repo_dir = assert_fs::TempDir::new()?;
        let _bare_repo = Repository::init_bare(&bare_repo_dir)?;
        // push to bare repository
        let repo_1 = Repository::open(&config_dir_1)?;
        let upstream_name = "remote";
        let mut repo_1_remote =
            repo_1.remote(upstream_name, bare_repo_dir.path().to_str().unwrap())?;
        repo_1_remote.push(&[repo_1.head().unwrap().name().unwrap()], None)?;
        trace!("bare repo {:?}", bare_repo_dir.display());
        println!("{:?}", bare_repo_dir.read_dir()?);
        // set up upstream branch
        let (mut repo_1_branch, _branch_type) = repo_1.branches(None)?.next().unwrap()?;
        repo_1_branch.set_upstream(Some(
            format!(
                "{}/{}",
                upstream_name,
                repo_1_branch.name().unwrap().unwrap()
            )
            .as_str(),
        ))?;

        // 2nd device
        //
        // devices: first, second
        let config_dir_2 = assert_fs::TempDir::new()?;
        setup_gitconfig()?;
        let mut cmd2 = Command::cargo_bin("xdbm")?;
        cmd2.arg("-c")
            .arg(config_dir_2.path())
            .arg("init")
            .arg("second")
            .arg("-r")
            .arg(bare_repo_dir.path());
        cmd2.assert().success().stdout(predicate::str::contains(""));

        assert_eq!(
            std::fs::read_to_string(config_dir_2.path().join("devname"))?,
            "second\n"
        );
        assert!(
            std::fs::read_to_string(config_dir_2.path().join("devices.yml"))?.contains("first")
        );
        assert!(
            std::fs::read_to_string(config_dir_2.path().join("devices.yml"))?.contains("second")
        );
        assert!(config_dir_2.join("backups").join("first.yml").exists());
        assert!(config_dir_2.join("backups").join("second.yml").exists());

        // sync
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("sync")
            .arg("-vvvv")
            .arg("-u")
            .assert()
            .success()
            .stderr(predicate::str::contains("successfully pushed"));
        std::process::Command::new("git")
            .arg("pull")
            .current_dir(&config_dir_1)
            .assert()
            .success();

        // Add storage
        //
        // devices: first, second
        // storages:
        //  - gdrive @ sample_storage (online)
        //      - first: sample_storage
        let sample_storage = assert_fs::TempDir::new()?;
        let mut cmd_add_storage_1 = Command::cargo_bin("xdbm")?;
        cmd_add_storage_1
            .arg("-c")
            .arg(config_dir_1.path())
            .arg("storage")
            .arg("add")
            .arg("online")
            .arg("--provider")
            .arg("google")
            .arg("--capacity")
            .arg("15000000000")
            .arg("--alias")
            .arg("gdrive")
            .arg("gdrive1")
            .arg(sample_storage.path());
        cmd_add_storage_1
            .assert()
            .success()
            .stdout(predicate::str::contains(""));
        // Add storage (directory)
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        let sample_directory = &sample_storage.join("foo").join("bar");
        DirBuilder::new().recursive(true).create(sample_directory)?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_1.path())
            .arg("storage")
            .arg("add")
            .arg("directory")
            .arg("--alias")
            .arg("docs")
            .arg("gdrive_docs")
            .arg(sample_directory)
            .assert()
            .success()
            .stdout(predicate::str::contains(""));
        assert!(
            std::fs::read_to_string(config_dir_1.join("storages.yml"))?.contains("parent: gdrive1")
        );

        run_sync_cmd(&config_dir_1, false)?;
        run_sync_cmd(&config_dir_2, false)?;

        // bind
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        //      - second: sample_directory
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("storage")
            .arg("bind")
            .arg("--alias")
            .arg("gdocs")
            .arg("--path")
            .arg(sample_directory)
            .arg("gdrive_docs")
            .assert()
            .success()
            .stdout(predicate::str::contains(""));

        // storage 3
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        //      - second: sample_directory
        //  - nas (online)
        //      - second: sample_storage_2
        let sample_storage_2 = assert_fs::TempDir::new()?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("storage")
            .arg("add")
            .arg("online")
            .arg("--provider")
            .arg("me")
            .arg("--capacity")
            .arg("1000000000000")
            .arg("--alias")
            .arg("nas")
            .arg("nas")
            .arg(sample_storage_2.path())
            .assert()
            .success();

        // storage list
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("storage")
            .arg("list")
            .arg("-l")
            .assert()
            .success()
            .stdout(predicate::str::contains("gdrive_docs").and(predicate::str::contains("nas")));

        // backup add
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        //      - second: sample_directory
        //  - nas (online)
        //      - second: sample_storage_2
        //  backups:
        //  - foodoc: second
        //      - sample_storage_2/foo/bar -> sample_directory/docs
        let backup_src = &sample_storage_2.join("foo").join("bar");
        DirBuilder::new().recursive(true).create(backup_src)?;
        let backup_dest = &sample_directory.join("docs");
        DirBuilder::new().recursive(true).create(backup_dest)?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("backup")
            .arg("add")
            .arg("--src")
            .arg(backup_src)
            .arg("--dest")
            .arg(backup_dest)
            .arg("foodoc")
            .arg("external")
            .arg("rsync")
            .arg("note: nonsense")
            .assert()
            .success();

        // backup add but with existing name
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("backup")
            .arg("add")
            .arg("--src")
            .arg(backup_src)
            .arg("--dest")
            .arg(backup_dest)
            .arg("foodoc")
            .arg("external")
            .arg("rsync")
            .arg("note: nonsense")
            .assert()
            .failure()
            .stderr(predicate::str::contains("already"));

        // backup list
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("backup")
            .arg("list")
            .assert()
            .success()
            .stdout(
                predicate::str::contains("foodoc")
                    .and(predicate::str::contains("nas"))
                    .and(predicate::str::contains("gdrive_docs"))
                    .and(predicate::str::contains("---")),
            );

        // backup done
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        //      - second: sample_directory
        //  - nas (online)
        //      - second: sample_storage_2
        //  backups:
        //  - foodoc: second
        //      - sample_storage_2/foo/bar -> sample_directory/docs (done 1)
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("backup")
            .arg("done")
            .arg("foodoc")
            .arg("0")
            .assert()
            .success();

        // backup list after backup done
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("backup")
            .arg("list")
            .assert()
            .success()
            .stdout(
                predicate::str::contains("foodoc")
                    .and(predicate::str::contains("nas"))
                    .and(predicate::str::contains("gdrive_docs"))
                    .and(predicate::str::contains("---").not()),
            );

        // status
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("status")
            .assert()
            .success();
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("status")
            .arg("-s")
            .arg(backup_src.clone().join("foo"))
            .assert()
            .success()
            .stdout(predicate::str::contains("nas").and(predicate::str::contains("foodoc").not()));
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("status")
            .arg("-sb")
            .arg(backup_src.clone().join("foo"))
            .assert()
            .success()
            .stdout(
                predicate::str::contains("nas")
                    .and(predicate::str::contains("second"))
                    .and(predicate::str::contains("foodoc")),
            );
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("status")
            .arg("-sb")
            .arg(backup_src.clone().parent().unwrap())
            .assert()
            .success()
            .stdout(
                predicate::str::contains("nas")
                    .and(predicate::str::contains("second").not())
                    .and(predicate::str::contains("foodoc").not()),
            );

        run_sync_cmd(&config_dir_2, true)?;
        run_sync_cmd(&config_dir_1, true)?;

        // bind
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        //      - second: sample_directory
        //  - nas (online)
        //      - first: sample_storage_2_first_path
        //      - second: sample_storage_2
        //  backups:
        //  - foodoc: second
        //      - sample_storage_2/foo/bar -> sample_directory/docs (done 1)
        let sample_storage_2_first_path = assert_fs::TempDir::new()?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_1.path())
            .arg("storage")
            .arg("bind")
            .arg("--alias")
            .arg("sample2")
            .arg("--path")
            .arg(sample_storage_2_first_path.path())
            .arg("nas")
            .assert()
            .success()
            .stdout(predicate::str::contains(""));

        // backup add
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        //      - second: sample_directory
        //  - nas (online)
        //      - first: sample_storage_2_first_path
        //      - second: sample_storage_2
        //  backups:
        //  - foodoc: second
        //      - sample_storage_2/foo/bar -> sample_directory/docs (done 1)
        //  - abcdbackup: first
        //      - sample_storage_2_first_path/abcd/efgh -> sample_storage/Downloads/abcd/efgh
        let backup_src = &sample_storage_2_first_path.join("abcd").join("efgh");
        DirBuilder::new().recursive(true).create(backup_src)?;
        let backup_dest = &sample_storage.join("Downloads").join("abcd").join("efgh");
        DirBuilder::new().recursive(true).create(backup_dest)?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_1.path())
            .arg("backup")
            .arg("add")
            .arg("--src")
            .arg(backup_src)
            .arg("--dest")
            .arg(backup_dest)
            .arg("abcdbackup")
            .arg("external")
            .arg("rsync")
            .arg("note: nonsense")
            .assert()
            .success();

        // backup add
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        //      - second: sample_directory
        //  - nas (online)
        //      - first: sample_storage_2_first_path
        //      - second: sample_storage_2
        //  backups:
        //  - foodoc: second
        //      - sample_storage_2/foo/bar -> sample_directory/docs (done 1)
        //  - abcdbackup: first
        //      - sample_storage_2_first_path/abcd/efgh -> sample_storage/Downloads/abcd/efgh
        //  - abcdsubbackup: first
        //      - sample_storage_2_first_path/abcd/efgh/sub -> sample_storage/Downloads/abcd/efgh/sub
        let backup_src = &sample_storage_2_first_path
            .join("abcd")
            .join("efgh")
            .join("sub");
        DirBuilder::new().recursive(true).create(backup_src)?;
        let backup_dest = &sample_storage
            .join("Downloads")
            .join("abcd")
            .join("efgh")
            .join("sub");
        DirBuilder::new().recursive(true).create(backup_dest)?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_1.path())
            .arg("backup")
            .arg("add")
            .arg("--src")
            .arg(backup_src)
            .arg("--dest")
            .arg(backup_dest)
            .arg("abcdsubbackup")
            .arg("external")
            .arg("rsync")
            .arg("note: only subdirectory")
            .assert()
            .success();

        run_sync_cmd(&config_dir_1, false)?;
        run_sync_cmd(&config_dir_2, false)?;

        // backup add
        //
        // devices: first, second
        // storages:
        //  - gdrive (online)
        //      - first: sample_storage
        //  - gdrive_docs (subdir of sample_storage/foo/bar)
        //      - first
        //      - second: sample_directory
        //  - nas (online)
        //      - first: sample_storage_2_first_path
        //      - second: sample_storage_2
        //  backups:
        //  - foodoc: second
        //      - sample_storage_2/foo/bar -> sample_directory/docs (done 1)
        //  - abcdbackup: first
        //      - sample_storage_2_first_path/abcd/efgh -> sample_storage/Downloads/abcd/efgh
        //  - abcdsubbackup: first
        //      - sample_storage_2_first_path/abcd/efgh/sub -> sample_storage/Downloads/abcd/efgh/sub
        //  - abcdbackup2: second
        //      - sample_storage_2/abcd/efgh -> sample_directory/Downloads/abcd/efgh
        let backup_src = &sample_storage_2.join("abcd").join("efgh");
        DirBuilder::new().recursive(true).create(backup_src)?;
        let backup_dest = &sample_directory.join("Downloads").join("abcd").join("efgh");
        DirBuilder::new().recursive(true).create(backup_dest)?;
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("backup")
            .arg("add")
            .arg("--src")
            .arg(backup_src)
            .arg("--dest")
            .arg(backup_dest)
            .arg("abcdbackup2")
            .arg("external")
            .arg("rsync")
            .arg("note: only subdirectory")
            .assert()
            .success();

        // status
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("status")
            .arg("-sb")
            .arg(backup_src)
            .assert()
            .success()
            .stdout(
                predicate::str::contains("nas")
                    .and(predicate::str::contains("first"))
                    .and(predicate::str::contains("abcdbackup"))
                    .and(predicate::str::contains("abcdsubbackup").not())
                    .and(predicate::str::contains("second"))
                    .and(predicate::str::contains("abcdbackup2")),
            );
        Command::cargo_bin("xdbm")?
            .arg("-c")
            .arg(config_dir_2.path())
            .arg("status")
            .arg("-sb")
            .arg(backup_src.join("sub"))
            .assert()
            .success()
            .stdout(
                predicate::str::contains("nas")
                    .and(predicate::str::contains("first"))
                    .and(predicate::str::contains("abcdbackup"))
                    .and(predicate::str::contains("abcdsubbackup"))
                    .and(predicate::str::contains("second"))
                    .and(predicate::str::contains("abcdbackup2")),
            );

        Ok(())
    }
}
