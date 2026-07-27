use super::*;

mod group_one;
mod group_three;
mod group_two;

fn test_repository() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("agent-room-test-{}", Uuid::new_v4()));
    let repository = root.join("repository");
    std::fs::create_dir_all(&repository).expect("create test repository");
    git(&repository, &["init".to_owned()]).expect("initialize repository");
    git(
        &repository,
        &[
            "config".to_owned(),
            "user.name".to_owned(),
            "Agent Room Test".to_owned(),
        ],
    )
    .expect("configure test name");
    git(
        &repository,
        &[
            "config".to_owned(),
            "user.email".to_owned(),
            "agent-room-test@local".to_owned(),
        ],
    )
    .expect("configure test email");
    std::fs::write(repository.join("plan.md"), "committed\n").expect("write tracked file");
    git_static(&repository, &["add", "plan.md"]).expect("stage tracked file");
    git_static(&repository, &["commit", "-m", "Initial"]).expect("commit tracked file");
    (root, repository)
}

fn remove_test_repository(root: &Path) {
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(root).expect("remove test repository");
}
