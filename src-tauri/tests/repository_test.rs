use multimodel_wiki_workbench_lib::domain::ProviderKind;
use multimodel_wiki_workbench_lib::repository::WorkspaceRepository;

#[tokio::test]
async fn persists_conversation_members_and_messages() {
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let conversation = repo.create_conversation("研究讨论").await.unwrap();

    repo.add_member(
        &conversation.id,
        ProviderKind::OpenAi,
        "gpt-5",
        "分析师",
        "从证据和约束出发分析问题",
    )
    .await
    .unwrap();
    repo.append_message(&conversation.id, "user", None, "比较两个方案")
        .await
        .unwrap();

    let thread = repo.load_thread(&conversation.id).await.unwrap();
    assert_eq!(thread.conversation.title, "研究讨论");
    assert_eq!(thread.members.len(), 1);
    assert_eq!(thread.members[0].provider, ProviderKind::OpenAi);
    assert_eq!(thread.messages.len(), 1);
    assert_eq!(thread.messages[0].content, "比较两个方案");
}

#[tokio::test]
async fn reopens_a_file_backed_workspace() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("workbench.sqlite");

    let conversation_id = {
        let repo = WorkspaceRepository::open(&database_path).await.unwrap();
        repo.create_conversation("长期会话").await.unwrap().id
    };

    let reopened = WorkspaceRepository::open(&database_path).await.unwrap();
    let thread = reopened.load_thread(&conversation_id).await.unwrap();

    assert_eq!(thread.conversation.title, "长期会话");
}
