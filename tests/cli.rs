use assert_cmd::prelude::*;
use assert_fs::prelude::*;

mod cmd_init {
    use anyhow::{Ok, Result};
    use assert_cmd::{cargo::CommandCargoExt, Command};
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

        // 2nd device
        let config_dir_2 = assert_fs::TempDir::new()?;
        let mut cmd2 = Command::cargo_bin("xdbm")?;
        cmd2.arg("-c")
            .arg(config_dir_2.path())
            .arg("init")
            .arg("second")
            .arg("-r")
            .arg(config_dir_1.path());
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
