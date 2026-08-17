import { invoke } from '@tauri-apps/api/core';

export type ProviderKind =
  | 'openai'
  | 'anthropic'
  | 'gemini'
  | 'deepseek'
  | 'qwen'
  | 'zhipu'
  | 'moonshot'
  | 'doubao';
export type StopReason = 'allSilent' | 'messageLimit' | 'userStopped';
export type ReviewStatus = 'pending' | 'accepted' | 'incorrect' | 'rolledBack';

export interface Conversation {
  id: string;
  title: string;
  createdAt: string;
}

export interface ModelMember {
  id: string;
  conversationId: string;
  provider: ProviderKind;
  model: string;
  roleName: string;
  roleInstruction: string;
}

export interface Message {
  id: string;
  conversationId: string;
  authorKind: 'user' | 'model' | 'system';
  authorId?: string;
  content: string;
  createdAt: string;
}

export interface DiscussionRecord {
  id: string;
  conversationId: string;
  triggerMessageId?: string;
  memberId?: string;
  kind: string;
  status: string;
  publicReason?: string;
  createdAt: string;
}

export interface SourceRecord {
  id: string;
  kind: string;
  title: string;
  sourceUri: string;
  rawPath: string;
  contentHash: string;
  extractedText?: string;
  extractionError?: string;
  createdAt: string;
}

export interface ConversationSnapshot {
  thread: {
    conversation: Conversation;
    members: ModelMember[];
    messages: Message[];
  };
  events: DiscussionRecord[];
  sources: SourceRecord[];
}

export interface ProviderStatus {
  provider: ProviderKind;
  configured: boolean;
}

export interface AddMemberInput {
  conversationId: string;
  provider: ProviderKind;
  model: string;
  roleName: string;
  roleInstruction: string;
}

export interface SendMessageInput {
  conversationId: string;
  content: string;
  mentionedMemberId?: string;
}

export interface MemberFailure {
  memberId: string;
  message: string;
}

export interface CycleState {
  modelMessageCount: number;
  stopReason: StopReason;
  failures: MemberFailure[];
}

export interface SourceInput {
  conversationId?: string;
  kind: 'file' | 'url';
  value: string;
}

export interface ReviewItem {
  id: string;
  revisionId: string;
  path: string;
  reason: string;
  status: ReviewStatus;
  sourceIds: string[];
  beforeContent?: string;
  afterContent: string;
  createdAt: string;
  reviewedAt?: string;
}

export interface WikiPage {
  path: string;
  title: string;
  summary: string;
  markdown: string;
}

export class AppError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = 'AppError';
    this.code = code;
  }
}

type WebState = {
  conversations: Conversation[];
  members: ModelMember[];
  messages: Message[];
  events: DiscussionRecord[];
  sources: SourceRecord[];
  sourceConversations: Record<string, string[]>;
  reviews: ReviewItem[];
  pages: WikiPage[];
  providers: ProviderStatus[];
};

const WEB_STORAGE_KEY = 'multimodel-wiki-workbench:web-state:v1';
const WEB_PROVIDERS: ProviderKind[] = [
  'openai',
  'anthropic',
  'gemini',
  'deepseek',
  'qwen',
  'zhipu',
  'moonshot',
  'doubao',
];

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    if (isErrorPayload(cause)) {
      throw new AppError(cause.code, cause.message);
    }
    if (isWebPreview()) {
      return webCall<T>(command, args ?? {});
    }
    throw new AppError('desktop_error', '桌面服务暂时不可用');
  }
}

function isWebPreview(): boolean {
  return typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);
}

function emptyWebState(): WebState {
  return {
    conversations: [],
    members: [],
    messages: [],
    events: [],
    sources: [],
    sourceConversations: {},
    reviews: [],
    pages: [],
    providers: WEB_PROVIDERS.map((provider) => ({ provider, configured: false })),
  };
}

function readWebState(): WebState {
  if (!isWebPreview()) return emptyWebState();
  try {
    const raw = window.localStorage.getItem(WEB_STORAGE_KEY);
    if (!raw) return emptyWebState();
    const parsed = JSON.parse(raw) as Partial<WebState>;
    const defaults = emptyWebState();
    const storedProviders = Array.isArray(parsed.providers) ? parsed.providers : [];
    return {
      ...defaults,
      ...parsed,
      sourceConversations: parsed.sourceConversations ?? defaults.sourceConversations,
      providers: WEB_PROVIDERS.map(
        (provider) => storedProviders.find((status) => status.provider === provider)
          ?? { provider, configured: false },
      ),
    };
  } catch {
    return emptyWebState();
  }
}

function writeWebState(state: WebState): void {
  try {
    window.localStorage.setItem(WEB_STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Browser privacy modes can disable localStorage; the current action still completes in memory.
  }
}

function webId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

async function webCall<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const state = readWebState();

  switch (command) {
    case 'create_conversation': {
      const title = typeof args.title === 'string' && args.title.trim() ? args.title.trim() : '未命名讨论';
      const conversation: Conversation = {
        id: webId('conversation'),
        title,
        createdAt: new Date().toISOString(),
      };
      state.conversations.unshift(conversation);
      writeWebState(state);
      return conversation as T;
    }
    case 'list_conversations':
      return [...state.conversations].sort((left, right) => right.createdAt.localeCompare(left.createdAt)) as T;
    case 'provider_statuses':
      return state.providers as T;
    case 'load_snapshot': {
      const conversationId = String(args.conversationId ?? '');
      const conversation = state.conversations.find((item) => item.id === conversationId);
      if (!conversation) throw new AppError('not_found', '会话不存在');
      return {
        thread: {
          conversation,
          members: state.members.filter((member) => member.conversationId === conversationId),
          messages: state.messages.filter((message) => message.conversationId === conversationId),
        },
        events: state.events.filter((event) => event.conversationId === conversationId),
        sources: state.sources.filter((source) => state.sourceConversations[source.id]?.includes(conversationId)),
      } as T;
    }
    case 'add_member': {
      const input = args.input as AddMemberInput;
      const member: ModelMember = { ...input, id: webId('member') };
      state.members.push(member);
      writeWebState(state);
      return member as T;
    }
    case 'send_message': {
      const input = args.input as SendMessageInput;
      const message: Message = {
        id: webId('message'),
        conversationId: input.conversationId,
        authorKind: 'user',
        content: input.content,
        createdAt: new Date().toISOString(),
      };
      state.messages.push(message);
      writeWebState(state);
      return { modelMessageCount: 0, stopReason: 'allSilent', failures: [] } as T;
    }
    case 'stop_discussion':
      return undefined as T;
    case 'ingest_source': {
      const input = args.input as SourceInput;
      const source: SourceRecord = {
        id: webId('source'),
        kind: input.kind,
        title: input.kind === 'url' ? input.value : input.value.split(/[\\/]/).pop() ?? '本地文件',
        sourceUri: input.kind === 'url' ? input.value : '',
        rawPath: input.value,
        contentHash: webId('hash'),
        extractedText: input.kind === 'url' ? '浏览器预览仅记录链接；桌面版可抓取网页正文。' : undefined,
        extractionError: input.kind === 'file' ? '浏览器预览不读取本地文件，请使用桌面版。' : undefined,
        createdAt: new Date().toISOString(),
      };
      state.sources.push(source);
      if (input.conversationId) state.sourceConversations[source.id] = [input.conversationId];
      writeWebState(state);
      return source as T;
    }
    case 'list_review_items':
      return state.reviews as T;
    case 'list_wiki_pages':
      return state.pages as T;
    case 'set_review_status': {
      const revisionId = String(args.revisionId ?? '');
      const review = state.reviews.find((item) => item.revisionId === revisionId);
      if (review) {
        review.status = args.status as ReviewStatus;
        review.reviewedAt = new Date().toISOString();
        writeWebState(state);
      }
      return undefined as T;
    }
    case 'rollback_revision': {
      const revisionId = String(args.revisionId ?? '');
      const review = state.reviews.find((item) => item.revisionId === revisionId);
      if (review) {
        review.status = 'rolledBack';
        review.reviewedAt = new Date().toISOString();
        writeWebState(state);
      }
      return undefined as T;
    }
    case 'save_provider_key':
    case 'delete_provider_key':
    case 'validate_provider':
      throw new AppError('web_unavailable', '浏览器预览不保存 API Key，请使用桌面版');
    default:
      throw new AppError('unsupported_command', `浏览器预览暂不支持：${command}`);
  }
}

function isErrorPayload(value: unknown): value is { code: string; message: string } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'message' in value &&
    typeof value.message === 'string'
  );
}

export const api = {
  saveProviderKey: (provider: ProviderKind, apiKey: string) =>
    call<void>('save_provider_key', { provider, apiKey }),
  deleteProviderKey: (provider: ProviderKind) =>
    call<void>('delete_provider_key', { provider }),
  validateProvider: (provider: ProviderKind) =>
    call<void>('validate_provider', { provider }),
  providerStatuses: () => call<ProviderStatus[]>('provider_statuses'),
  createConversation: (title: string) =>
    call<Conversation>('create_conversation', { title }),
  listConversations: () => call<Conversation[]>('list_conversations'),
  addMember: (input: AddMemberInput) => call<ModelMember>('add_member', { input }),
  loadSnapshot: (conversationId: string) =>
    call<ConversationSnapshot>('load_snapshot', { conversationId }),
  sendMessage: (input: SendMessageInput) => call<CycleState>('send_message', { input }),
  stopDiscussion: (conversationId: string) =>
    call<void>('stop_discussion', { conversationId }),
  ingestSource: (input: SourceInput) => call<SourceRecord>('ingest_source', { input }),
  listReviewItems: () => call<ReviewItem[]>('list_review_items'),
  listWikiPages: () => call<WikiPage[]>('list_wiki_pages'),
  setReviewStatus: (revisionId: string, status: ReviewStatus) =>
    call<void>('set_review_status', { revisionId, status }),
  rollbackRevision: (revisionId: string) =>
    call<void>('rollback_revision', { revisionId }),
};
