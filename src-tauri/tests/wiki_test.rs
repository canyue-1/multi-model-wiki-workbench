use std::fs;
use std::path::PathBuf;

use multimodel_wiki_workbench_lib::domain::ReviewStatus;
use multimodel_wiki_workbench_lib::repository::WorkspaceRepository;
use multimodel_wiki_workbench_lib::wiki::{WikiChange, WikiError, WikiService};
use tempfile::tempdir;

#[tokio::test]
async fn applies_revision_and_can_roll_it_back() {
    let workspace = tempdir().unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let wiki = WikiService::new(workspace.path(), repo.clone());

    let revision = wiki
        .apply(change(
            "topics/模型路由.md",
            "# 模型路由\n\n根据任务选择模型。\n",
        ))
        .await
        .unwrap();

    assert!(revision.review_pending);
    assert_eq!(revision.source_ids, vec!["source-1"]);
    let page = workspace.path().join("wiki/topics/模型路由.md");
    assert_eq!(
        fs::read_to_string(&page).unwrap(),
        "# 模型路由\n\n根据任务选择模型。\n"
    );
    let index = fs::read_to_string(workspace.path().join("wiki/index.md")).unwrap();
    assert!(index.contains("[模型路由](topics/模型路由.md)"));
    assert!(index.contains("根据任务选择模型"));
    let log_before = fs::read_to_string(workspace.path().join("wiki/log.md")).unwrap();
    assert!(log_before.contains("apply | topics/模型路由.md"));
    assert!(log_before.contains("source-1"));
    assert_eq!(repo.list_review_items().await.unwrap().len(), 1);

    wiki.rollback(&revision.id).await.unwrap();

    assert!(!page.exists());
    let index = fs::read_to_string(workspace.path().join("wiki/index.md")).unwrap();
    assert!(!index.contains("模型路由"));
    let log_after = fs::read_to_string(workspace.path().join("wiki/log.md")).unwrap();
    assert!(log_after.starts_with(&log_before));
    assert!(log_after.contains("rollback | topics/模型路由.md"));
    let reviews = repo.list_review_items().await.unwrap();
    assert_eq!(reviews[0].status, ReviewStatus::RolledBack);
}

#[tokio::test]
async fn update_rollback_restores_the_previous_markdown() {
    let workspace = tempdir().unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let wiki = WikiService::new(workspace.path(), repo);
    wiki.apply(change("topics/context.md", "# Context\n\nVersion one.\n"))
        .await
        .unwrap();
    let update = wiki
        .apply(change("topics/context.md", "# Context\n\nVersion two.\n"))
        .await
        .unwrap();

    wiki.rollback(&update.id).await.unwrap();

    assert_eq!(
        fs::read_to_string(workspace.path().join("wiki/topics/context.md")).unwrap(),
        "# Context\n\nVersion one.\n"
    );
}

#[tokio::test]
async fn review_items_can_be_accepted_or_marked_incorrect() {
    let workspace = tempdir().unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let wiki = WikiService::new(workspace.path(), repo.clone());
    let accepted = wiki
        .apply(change("sources/accepted.md", "# Accepted\n"))
        .await
        .unwrap();
    let incorrect = wiki
        .apply(change("sources/incorrect.md", "# Incorrect\n"))
        .await
        .unwrap();

    wiki.set_review_status(&accepted.id, ReviewStatus::Accepted)
        .await
        .unwrap();
    wiki.set_review_status(&incorrect.id, ReviewStatus::Incorrect)
        .await
        .unwrap();

    let reviews = repo.list_review_items().await.unwrap();
    assert!(
        reviews
            .iter()
            .any(|item| item.revision_id == accepted.id && item.status == ReviewStatus::Accepted)
    );
    assert!(
        reviews
            .iter()
            .any(|item| item.revision_id == incorrect.id && item.status == ReviewStatus::Incorrect)
    );
}

#[tokio::test]
async fn rejects_paths_outside_the_wiki() {
    let workspace = tempdir().unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let wiki = WikiService::new(workspace.path(), repo);

    let error = wiki
        .apply(change("../raw/source.md", "# escaped\n"))
        .await
        .unwrap_err();

    assert!(matches!(error, WikiError::InvalidPath(_)));
    assert!(!workspace.path().join("raw/source.md").exists());
}

#[tokio::test]
async fn refuses_to_rollback_a_revision_superseded_by_a_newer_edit() {
    let workspace = tempdir().unwrap();
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let wiki = WikiService::new(workspace.path(), repo);
    let first = wiki
        .apply(change("topics/history.md", "# History\n\nFirst.\n"))
        .await
        .unwrap();
    wiki.apply(change("topics/history.md", "# History\n\nSecond.\n"))
        .await
        .unwrap();

    let error = wiki.rollback(&first.id).await.unwrap_err();

    assert!(matches!(error, WikiError::StaleRevision));
    assert_eq!(
        fs::read_to_string(workspace.path().join("wiki/topics/history.md")).unwrap(),
        "# History\n\nSecond.\n"
    );
}

fn change(path: &str, markdown: &str) -> WikiChange {
    WikiChange {
        relative_path: PathBuf::from(path),
        markdown: markdown.to_owned(),
        source_ids: vec!["source-1".to_owned()],
        reason: "综合新资料".to_owned(),
    }
}
