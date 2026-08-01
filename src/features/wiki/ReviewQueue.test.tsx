import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { ReviewQueue } from './ReviewQueue';

const item = {
  id: 'review-1',
  revisionId: 'revision-1',
  path: 'topics/模型路由.md',
  reason: '综合新资料',
  status: 'pending' as const,
  sourceIds: ['source-1'],
  afterContent: '# 模型路由',
  createdAt: '2026-08-01 10:00:00',
};

describe('ReviewQueue', () => {
  it('offers accept, incorrect, and rollback actions', () => {
    const onAccept = vi.fn();
    const onIncorrect = vi.fn();
    const onRollback = vi.fn();
    render(
      <ReviewQueue
        items={[item]}
        busyRevisionId={null}
        onAccept={onAccept}
        onIncorrect={onIncorrect}
        onRollback={onRollback}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: '接受修订' }));
    fireEvent.click(screen.getByRole('button', { name: '标记错误' }));
    fireEvent.click(screen.getByRole('button', { name: '回退修订' }));

    expect(onAccept).toHaveBeenCalledWith('revision-1');
    expect(onIncorrect).toHaveBeenCalledWith('revision-1');
    expect(onRollback).toHaveBeenCalledWith('revision-1');
  });
});
