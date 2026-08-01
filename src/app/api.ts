import { invoke } from '@tauri-apps/api/core';

export type ProviderKind = 'openai' | 'anthropic' | 'gemini' | 'deepseek';
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

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    if (isErrorPayload(cause)) {
      throw new AppError(cause.code, cause.message);
    }
    throw new AppError('desktop_error', '桌面服务暂时不可用');
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
