# 多模型 Wiki 工作台实施计划

> **给执行代理：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐任务执行本计划。使用复选框跟踪步骤。

**目标：** 交付一个本地 Tauri 桌面应用的端到端薄切片，支持多模型自然群聊、资料导入、自动维护 Wiki 与修订复核。

**架构：** React/TypeScript 负责工作台界面，Rust/Tauri 负责密钥、模型调用、文件系统和 SQLite。所有供应商通过统一适配接口接入，讨论调度器只依赖该接口；原始资料保存到 `raw/`，Wiki 保存为 Markdown，运行状态保存到 SQLite。

**技术栈：** Tauri 2、React 19、TypeScript、Vite、Vitest、Testing Library、Rust、Tokio、Reqwest、SQLx/SQLite、Serde、keyring、pulldown-cmark。

---

## 文件结构

```text
package.json                     前端脚本与依赖
src/
  app/App.tsx                    应用外壳与工作区路由
  app/api.ts                     Tauri 命令的类型化封装
  features/chat/                 群聊工作台、状态与组件
  features/providers/            供应商与模型配置界面
  features/sources/              资料导入界面
  features/wiki/                 Wiki 浏览与待复核界面
  test/setup.ts                  前端测试初始化
src-tauri/
  Cargo.toml                     Rust 依赖
  migrations/0001_init.sql       SQLite 初始结构
  src/lib.rs                     Tauri 初始化与命令注册
  src/domain.rs                  跨组件领域类型
  src/repository.rs              SQLite 仓库
  src/secrets.rs                 系统密钥库
  src/providers/                 统一模型接口与四家适配器
  src/scheduler.rs               自然群聊事件调度器
  src/sources.rs                 文件、网页与文本提取
  src/wiki.rs                    Wiki 写入、索引、日志与回退
  src/commands.rs                Tauri 命令边界
  tests/                         Rust 集成测试
```

## 任务 1：建立可测试的 Tauri/React 应用骨架

**文件：**
- 新建：`package.json`、`vite.config.ts`、`index.html`、`src/main.tsx`、`src/app/App.tsx`
- 新建：`src/test/setup.ts`、`src/app/App.test.tsx`
- 新建：`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src-tauri/src/main.rs`、`src-tauri/src/lib.rs`

- [ ] **步骤 1：先写前端冒烟测试**

```tsx
// src/app/App.test.tsx
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { App } from './App';

describe('App', () => {
  it('renders the local workspace shell', () => {
    render(<App />);
    expect(screen.getByRole('heading', { name: '多模型 Wiki 工作台' })).toBeVisible();
  });
});
```

- [ ] **步骤 2：运行测试并确认因缺少应用骨架而失败**

运行：`pnpm test -- --run src/app/App.test.tsx`  
预期：FAIL，提示无法解析 `./App` 或缺少测试脚本。

- [ ] **步骤 3：创建最小前端和 Tauri 入口**

```tsx
// src/app/App.tsx
export function App() {
  return <main><h1>多模型 Wiki 工作台</h1></main>;
}
```

`package.json` 固定脚本为 `dev: vite`、`test: vitest`、`build: tsc && vite build`、`tauri: tauri`；Rust `run()` 仅注册 Tauri builder 并启动应用。

- [ ] **步骤 4：验证前端测试、构建和 Rust 编译**

运行：`pnpm test -- --run`，预期：PASS。  
运行：`pnpm build`，预期：成功生成 `dist/`。  
运行：`cargo check --manifest-path src-tauri/Cargo.toml`，预期：退出码 0。

- [ ] **步骤 5：提交骨架**

```bash
git add package.json pnpm-lock.yaml vite.config.ts index.html src src-tauri
git commit -m "chore: scaffold tauri react workbench"
```

## 任务 2：定义领域模型和 SQLite 仓库

**文件：**
- 新建：`src-tauri/src/domain.rs`、`src-tauri/src/repository.rs`
- 新建：`src-tauri/migrations/0001_init.sql`
- 新建：`src-tauri/tests/repository_test.rs`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：写仓库失败测试**

```rust
#[tokio::test]
async fn persists_conversation_members_and_messages() {
    let repo = TestRepository::in_memory().await;
    let conversation = repo.create_conversation("研究讨论").await.unwrap();
    repo.add_member(conversation.id, "openai", "gpt-5", "分析师").await.unwrap();
    repo.append_message(conversation.id, "user", "比较两个方案").await.unwrap();
    assert_eq!(repo.load_thread(conversation.id).await.unwrap().messages.len(), 1);
}
```

- [ ] **步骤 2：运行单测并确认缺少仓库类型**

运行：`cargo test --manifest-path src-tauri/Cargo.toml repository_test`  
预期：FAIL，提示 `TestRepository` 或仓库方法未定义。

- [ ] **步骤 3：实现最小领域类型和迁移**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProviderKind { OpenAi, Anthropic, Gemini, DeepSeek }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelMember {
    pub id: String,
    pub provider: ProviderKind,
    pub model: String,
    pub role_name: String,
    pub role_instruction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpeakerDecision { Reply { reason: String, priority: i32 }, Silent { reason: String } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContext { pub conversation_id: String, pub trigger_message_id: String, pub member: ModelMember }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyContext { pub conversation_id: String, pub member: ModelMember, pub visible_messages: Vec<Message> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelReply { pub content: String, pub cited_source_ids: Vec<String> }
```

迁移必须创建 `conversations`、`members`、`messages`、`events`、`sources`、`citations`、`wiki_revisions` 和 `review_items`，并为外键及常用时间查询建立索引。

- [ ] **步骤 4：实现 SQLx 仓库并通过测试**

运行：`cargo test --manifest-path src-tauri/Cargo.toml repository_test`  
预期：PASS，且临时 SQLite 文件关闭后可重新打开并加载同一会话。

- [ ] **步骤 5：提交数据层**

```bash
git add src-tauri/migrations src-tauri/src/domain.rs src-tauri/src/repository.rs src-tauri/tests/repository_test.rs src-tauri/src/lib.rs
git commit -m "feat: add local workspace repository"
```

## 任务 3：实现密钥库和统一模型适配层

**文件：**
- 新建：`src-tauri/src/secrets.rs`
- 新建：`src-tauri/src/providers/{mod.rs,openai.rs,anthropic.rs,gemini.rs,deepseek.rs}`
- 新建：`src-tauri/tests/provider_contract_test.rs`

- [ ] **步骤 1：写供应商契约失败测试**

```rust
#[tokio::test]
async fn adapters_normalize_speaker_decisions() {
    let adapter = FakeAdapter::returning_json(r#"{"decision":"silent","reason":"没有新增信息"}"#);
    let result = adapter.decide(test_context()).await.unwrap();
    assert_eq!(result, SpeakerDecision::Silent { reason: "没有新增信息".into() });
}
```

- [ ] **步骤 2：确认契约测试失败**

运行：`cargo test --manifest-path src-tauri/Cargo.toml provider_contract_test`  
预期：FAIL，提示 `ModelProvider` 未定义。

- [ ] **步骤 3：实现统一接口和错误分类**

```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn validate_key(&self) -> Result<(), ProviderError>;
    async fn decide(&self, context: DecisionContext) -> Result<SpeakerDecision, ProviderError>;
    async fn reply(&self, context: ReplyContext) -> Result<ModelReply, ProviderError>;
}

pub enum ProviderError { InvalidKey, Quota, Timeout, Transport, MalformedDecision, Remote(String) }
```

四个适配器只负责鉴权头、请求体、响应提取和错误映射；结构化决策统一解析为 `SpeakerDecision`。瞬时传输错误最多重试一次，生成结果不自动重试。

- [ ] **步骤 4：实现系统密钥库包装**

```rust
pub trait SecretStore {
    fn save(&self, provider: ProviderKind, value: &str) -> Result<(), SecretError>;
    fn load(&self, provider: ProviderKind) -> Result<Option<String>, SecretError>;
    fn delete(&self, provider: ProviderKind) -> Result<(), SecretError>;
}
```

测试使用内存实现；生产实现使用 `keyring`，不得把密钥写入 SQLite 或日志。

- [ ] **步骤 5：运行契约测试并提交**

运行：`cargo test --manifest-path src-tauri/Cargo.toml provider_contract_test`，预期：PASS。  
运行：`cargo test --manifest-path src-tauri/Cargo.toml`，预期：全部 PASS。

```bash
git add src-tauri/src/secrets.rs src-tauri/src/providers src-tauri/tests/provider_contract_test.rs
git commit -m "feat: add secure model provider adapters"
```

## 任务 4：实现自然群聊调度器

**文件：**
- 新建：`src-tauri/src/scheduler.rs`
- 新建：`src-tauri/tests/scheduler_test.rs`
- 修改：`src-tauri/src/domain.rs`、`src-tauri/src/repository.rs`

- [ ] **步骤 1：写停止、冷却和故障隔离测试**

```rust
#[tokio::test]
async fn stops_after_twelve_model_messages() {
    let result = run_with_always_replying_models(3).await;
    assert_eq!(result.model_message_count, 12);
    assert_eq!(result.stop_reason, StopReason::MessageLimit);
}

#[tokio::test]
async fn one_provider_failure_does_not_stop_others() {
    let result = run_with_one_failure_and_one_reply().await;
    assert_eq!(result.messages.len(), 1);
    assert_eq!(result.failures.len(), 1);
}
```

- [ ] **步骤 2：确认测试失败**

运行：`cargo test --manifest-path src-tauri/Cargo.toml scheduler_test`  
预期：FAIL，提示 `DiscussionScheduler` 未定义。

- [ ] **步骤 3：实现事件循环**

```rust
pub async fn handle_event(&self, event: DiscussionEvent) -> Result<CycleState, SchedulerError> {
    let eligible = self.eligible_members(&event).await?;
    let decisions = self.collect_decisions(eligible, &event).await;
    self.record_public_reasons(&decisions).await?;
    self.enqueue_replies(decisions).await?;
    self.drain_queue_until_silent_or_limit(12).await
}
```

同时在 `domain.rs` 定义 `DiscussionEvent { conversation_id, trigger_message_id, mentioned_member_id }`、`CycleState { model_message_count, stop_reason, failures }` 与 `StopReason::{AllSilent, MessageLimit, UserStopped}`，供仓库、命令层和前端共享序列化名称。

队列按优先级降序和成员稳定 ID 排序；作者不参与本次决策；同一模型不得连续发言；明确点名可绕过冷却；全部沉默立即结束。

- [ ] **步骤 4：运行调度器和全套 Rust 测试**

运行：`cargo test --manifest-path src-tauri/Cargo.toml scheduler_test`，预期：PASS。  
运行：`cargo test --manifest-path src-tauri/Cargo.toml`，预期：全部 PASS。

- [ ] **步骤 5：提交调度器**

```bash
git add src-tauri/src/scheduler.rs src-tauri/src/domain.rs src-tauri/src/repository.rs src-tauri/tests/scheduler_test.rs
git commit -m "feat: add natural discussion scheduler"
```

## 任务 5：实现资料导入和不可变原始库

**文件：**
- 新建：`src-tauri/src/sources.rs`
- 新建：`src-tauri/tests/source_ingestion_test.rs`
- 修改：`src-tauri/src/repository.rs`

- [ ] **步骤 1：写文件和网页快照测试**

```rust
#[tokio::test]
async fn copies_source_without_modifying_original() {
    let result = ingestor.ingest_file(fixture("note.md")).await.unwrap();
    assert!(result.raw_path.ends_with("raw/note.md"));
    assert_eq!(result.extracted_text.as_deref(), Some("# 资料"));
    assert_eq!(read_fixture("note.md"), "# 资料");
}
```

- [ ] **步骤 2：确认测试失败后实现导入接口**

```rust
pub trait SourceIngestor {
    async fn ingest_file(&self, path: &Path) -> Result<IngestedSource, IngestError>;
    async fn capture_url(&self, url: &Url) -> Result<IngestedSource, IngestError>;
}
```

按内容哈希生成防冲突文件名；支持 TXT/Markdown、DOCX、文本型 PDF 和常见图片元数据；网页保存 HTML 快照、正文和抓取时间。解析失败仍保存原件并记录 `extraction_error`。

- [ ] **步骤 3：验证格式矩阵与失败路径**

运行：`cargo test --manifest-path src-tauri/Cargo.toml source_ingestion_test`  
预期：支持格式 PASS；不支持格式返回明确错误；原始 fixture 未改变。

- [ ] **步骤 4：提交资料导入**

```bash
git add src-tauri/src/sources.rs src-tauri/src/repository.rs src-tauri/tests/source_ingestion_test.rs
git commit -m "feat: add immutable source ingestion"
```

## 任务 6：实现 Wiki 维护、索引、日志和回退

**文件：**
- 新建：`src-tauri/src/wiki.rs`
- 新建：`src-tauri/tests/wiki_test.rs`
- 修改：`src-tauri/src/repository.rs`

- [ ] **步骤 1：写修订和回退失败测试**

```rust
#[tokio::test]
async fn applies_revision_and_can_roll_it_back() {
    let revision = wiki.apply(change("topics/模型路由.md", "# 模型路由\n", vec![source_id()])).await.unwrap();
    assert!(revision.review_pending);
    wiki.rollback(revision.id).await.unwrap();
    assert!(!workspace.join("wiki/topics/模型路由.md").exists());
}
```

- [ ] **步骤 2：实现原子写入与修订模型**

```rust
pub struct WikiChange {
    pub relative_path: PathBuf,
    pub markdown: String,
    pub source_ids: Vec<String>,
    pub reason: String,
}

pub async fn apply(&self, change: WikiChange) -> Result<WikiRevision, WikiError>;
pub async fn rollback(&self, revision_id: String) -> Result<(), WikiError>;
```

写入先落临时文件再原子替换；保存前后内容和哈希；每次变更同步更新 `index.md`、追加 `log.md`，并创建 `pending` 复核项。

- [ ] **步骤 3：运行 Wiki 测试**

运行：`cargo test --manifest-path src-tauri/Cargo.toml wiki_test`  
预期：创建、更新、引用、索引、日志、复核和回退全部 PASS。

- [ ] **步骤 4：提交 Wiki 服务**

```bash
git add src-tauri/src/wiki.rs src-tauri/src/repository.rs src-tauri/tests/wiki_test.rs
git commit -m "feat: add revisioned wiki maintenance"
```

## 任务 7：暴露类型安全的 Tauri 命令

**文件：**
- 新建：`src-tauri/src/commands.rs`、`src/app/api.ts`
- 新建：`src-tauri/tests/command_test.rs`、`src/app/api.test.ts`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：写命令边界测试**

```ts
it('maps invoke failures to an actionable app error', async () => {
  invokeMock.mockRejectedValue({ code: 'invalid_key', message: '密钥无效' });
  await expect(api.validateProvider('openai')).rejects.toMatchObject({ code: 'invalid_key' });
});
```

- [ ] **步骤 2：实现命令 DTO 和前端封装**

```ts
export type ProviderKind = 'openai' | 'anthropic' | 'gemini' | 'deepseek';
export interface SendMessageInput { conversationId: string; content: string; mentionedMemberId?: string }
export interface CycleState { modelMessageCount: number; stopReason: 'allSilent' | 'messageLimit' | 'userStopped' }
export interface SourceInput { conversationId?: string; kind: 'file' | 'url'; value: string }
export interface SourceRecord { id: string; title: string; rawPath: string; extractionError?: string }
export interface ReviewItem { id: string; revisionId: string; path: string; reason: string; status: 'pending' | 'accepted' | 'incorrect' | 'rolledBack' }

export const api = {
  validateProvider: (provider: ProviderKind) => invoke<void>('validate_provider', { provider }),
  sendMessage: (input: SendMessageInput) => invoke<CycleState>('send_message', { input }),
  ingestSource: (input: SourceInput) => invoke<SourceRecord>('ingest_source', { input }),
  listReviewItems: () => invoke<ReviewItem[]>('list_review_items'),
  rollbackRevision: (revisionId: string) => invoke<void>('rollback_revision', { revisionId }),
};
```

Rust 命令只做验证、权限边界和错误序列化；业务逻辑委托给已有服务。所有路径在进入服务前验证属于当前工作区。

- [ ] **步骤 3：运行前后端命令测试并提交**

运行：`pnpm test -- --run src/app/api.test.ts`，预期：PASS。  
运行：`cargo test --manifest-path src-tauri/Cargo.toml command_test`，预期：PASS。

```bash
git add src/app/api.ts src/app/api.test.ts src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/tests/command_test.rs
git commit -m "feat: expose typed desktop commands"
```

## 任务 8：实现桌面工作台界面

**文件：**
- 新建：`src/features/providers/ProviderSettings.tsx`
- 新建：`src/features/chat/{ChatWorkbench.tsx,ModelRoster.tsx,MessageTimeline.tsx,Composer.tsx,useDiscussion.ts}`
- 新建：`src/features/sources/SourcePanel.tsx`
- 新建：`src/features/wiki/{WikiPanel.tsx,ReviewQueue.tsx}`
- 新建：上述组件对应的 `*.test.tsx`
- 修改：`src/app/App.tsx`、`src/styles.css`

- [ ] **步骤 1：写用户主流程失败测试**

```tsx
it('shows model decisions and allows stopping a cycle', async () => {
  render(<ChatWorkbench conversationId="c1" />);
  await user.type(screen.getByRole('textbox', { name: '消息' }), '讨论这个资料');
  await user.click(screen.getByRole('button', { name: '发送' }));
  expect(await screen.findByText('没有新增信息，保持沉默')).toBeVisible();
  expect(screen.getByRole('button', { name: '停止讨论' })).toBeEnabled();
});
```

- [ ] **步骤 2：实现三栏工作台**

左栏展示会话和模型成员；中栏展示消息、公开理由、错误状态与编辑器；右栏展示当前资料、Wiki 页面和待复核项。所有异步状态使用明确文本，不只依赖颜色。

```tsx
export function ChatWorkbench({ conversationId }: Props) {
  const discussion = useDiscussion(conversationId);
  return <div className="workbench">
    <ModelRoster members={discussion.members} />
    <MessageTimeline events={discussion.events} />
    <SourcePanel sources={discussion.sources} />
    <Composer onSend={discussion.send} onStop={discussion.stop} />
  </div>;
}
```

- [ ] **步骤 3：实现供应商配置、资料导入和复核操作**

配置页面不得回显已保存密钥；资料导入显示提取成功或失败；待复核项展示路径、理由、来源和差异，并提供接受、标记错误和回退按钮。

- [ ] **步骤 4：运行组件测试、构建并提交**

运行：`pnpm test -- --run`，预期：全部 PASS。  
运行：`pnpm build`，预期：退出码 0。

```bash
git add src
git commit -m "feat: build multi-model desktop workbench"
```

## 任务 9：完成端到端薄切片和安全验收

**文件：**
- 新建：`src-tauri/tests/end_to_end_test.rs`
- 新建：`src-tauri/tests/fixtures/two-model-source-discussion.json`
- 新建：`src-tauri/tests/fixtures/source.md`
- 新建：`docs/testing/acceptance-checklist.md`
- 修改：`src-tauri/tauri.conf.json`

- [ ] **步骤 1：写端到端测试**

测试使用四个可控的模拟适配器，完成：创建会话、加入两个模型、附加 Markdown、发送消息、一个模型回复、一个模型沉默、生成 Wiki、查看待复核项、回退修订、重启后恢复会话。

```rust
#[tokio::test]
async fn completes_group_chat_to_reviewable_wiki_flow() {
    let app = TestApp::new().await;
    let outcome = app.run_fixture("two-model-source-discussion").await.unwrap();
    assert_eq!(outcome.model_messages, 1);
    assert_eq!(outcome.silent_decisions, 1);
    assert_eq!(outcome.pending_reviews, 1);
    assert!(outcome.wiki_page.exists());
}
```

- [ ] **步骤 2：添加密钥泄露回归检查**

在测试工作区中使用唯一哨兵密钥，递归扫描 SQLite 导出文本、日志、Markdown 和 `raw/`；任一出现哨兵字符串即失败。

- [ ] **步骤 3：运行完整验证矩阵**

运行：`pnpm test -- --run`，预期：全部 PASS。  
运行：`pnpm build`，预期：PASS。  
运行：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`，预期：无差异。  
运行：`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`，预期：退出码 0。  
运行：`cargo test --manifest-path src-tauri/Cargo.toml`，预期：全部 PASS。  
运行：`pnpm tauri build`，预期：生成可安装桌面包。

- [ ] **步骤 4：执行人工验收清单**

验证有效/无效密钥、断网、超时、全部沉默、多个候选同时发言、12 条上限、停止/继续、模型点名、关闭重开、解析失败、Wiki 复核和回退。

- [ ] **步骤 5：提交端到端薄切片**

```bash
git add src-tauri/tests docs/testing
git commit -m "test: verify end-to-end workbench flow"
```

## 完成条件

- 设计规格的八条验收标准均有自动化测试或人工验收项对应。
- 四家供应商通过同一接口接入；调度器和 UI 不含供应商特例。
- API Key 仅存在于操作系统凭据库与测试内存实现中。
- 原始资料不可变，Wiki 可追溯、可复核、可回退。
- `pnpm test -- --run`、`pnpm build`、Rust 格式、Clippy、测试与 `pnpm tauri build` 全部通过。
