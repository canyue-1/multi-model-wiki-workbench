import type { ProviderKind } from '../../app/api';

export const PROVIDERS: Array<{
  kind: ProviderKind;
  label: string;
  shortLabel: string;
  defaultModel: string;
}> = [
  { kind: 'openai', label: 'OpenAI', shortLabel: 'OA', defaultModel: 'gpt-5-mini' },
  { kind: 'anthropic', label: 'Anthropic', shortLabel: 'AN', defaultModel: 'claude-sonnet-4-5' },
  { kind: 'gemini', label: 'Google Gemini', shortLabel: 'GE', defaultModel: 'gemini-2.5-flash' },
  { kind: 'deepseek', label: 'DeepSeek', shortLabel: 'DS', defaultModel: 'deepseek-chat' },
  { kind: 'qwen', label: '通义千问', shortLabel: '通', defaultModel: 'qwen-plus' },
  { kind: 'zhipu', label: '智谱 GLM', shortLabel: '智', defaultModel: 'glm-4.5' },
  { kind: 'moonshot', label: 'Kimi', shortLabel: 'KM', defaultModel: 'moonshot-v1-8k' },
  { kind: 'doubao', label: '豆包', shortLabel: '豆', defaultModel: 'doubao-1-5-pro-32k-250115' },
];

export const ROLE_PRESETS = [
  { name: '分析师', instruction: '拆解问题，比较方案，并给出结构化判断。' },
  { name: '质疑者', instruction: '检查假设、风险和论证缺口，提出反例。' },
  { name: '创意者', instruction: '寻找非显然的连接，提出可落地的新方向。' },
  { name: '事实核查者', instruction: '核对事实依据，区分证据、推断与未知。' },
];

export function providerLabel(provider: ProviderKind): string {
  return PROVIDERS.find((item) => item.kind === provider)?.label ?? provider;
}

export function defaultModel(provider: ProviderKind): string {
  return PROVIDERS.find((item) => item.kind === provider)?.defaultModel ?? '';
}
