use assert_cmd::prelude::*;
use assert_fs::prelude::*;

mod cmd_init {
    use anyhow::{Ok, Result};
    use assert_cmd::Command;
    use git2::Repository;
    use log::trace;
    use predicates::prelude::predicate;

    #[test]
    fn init_with_tmpdir() -> Result<()> {
        let config_dir = assert_fs::TempDir::new()?;
        let mut cmd = Command::cargo_bin("xdbm")?;
        cmd.arg("-c")
            .arg(config_dir.path())
            .arg("init")
            .arg("testdev");
        cmd.assert().success().stdout(predicate::str::contains(""));
        assert_eq!(
            std::fs::read_to_string(config_dir.path().join("devname"))?,
            "testdev\n"
        );
        Ok(())
    }

    #[test]
    fn init_with_existing_repo() -> Result<()> {
        // 1st device
        let config_dir_1 = assert_fs::TempDir::new()?;
        let mut cmd1 = Command::cargo_bin("xdbm")?;
        cmd1.arg("-c")
            .arg(config_dir_1.path())
            .arg("init")
            .arg("first");
        cmd1.assert().success().stdout(predicate::str::contains(""));

        // bare-repo
        let bare_repo_dir = assert_fs::TempDir::new()?;
        let bare_repo = Repository::init_bare(&bare_repo_dir)?;
        let repo_1 = Repository::open(&config_dir_1)?;
        let upstream_name = "remote";
        let mut repo_1_remote =
            repo_1.remote(upstream_name, &bare_repo_dir.path().to_str().unwrap())?;
        repo_1_remote.push(&["refs/heads/main"], None)?;
        trace!("bare repo {:?}", bare_repo_dir.display());
        println!("{:?}", bare_repo_dir.read_dir()?);
        // set up upstream branch
        let (mut repo_1_branch, _branch_type) = repo_1.branches(None)?.next().unwrap()?;
        repo_1_branch.set_upstream(Some(format!("{}/{}", upstream_name, "main").as_str()))?;

        // 2nd device
        let config_dir_2 = assert_fs::TempDir::new()?;
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
        Ok(())
    }
}
