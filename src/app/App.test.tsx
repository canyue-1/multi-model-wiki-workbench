import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { App } from './App';

describe('App', () => {
  it('renders the local workspace shell', () => {
    render(<App />);

    expect(
      screen.getByRole('heading', { name: '多模型 Wiki 工作台' }),
    ).toBeVisible();
  });
});
