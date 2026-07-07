import { describe, expect, it } from 'vitest';
import { ApiError } from './api/sequencer';
import { friendlyError } from './errors';

describe('friendlyError never renders "[object Object]"', () => {
  it('handles Freighter-style error objects', () => {
    // Freighter v4+ rejects with { code, message } objects, not Errors.
    expect(friendlyError({ code: -4, message: 'The user rejected this request.' })).toBe(
      'The user rejected this request.',
    );
  });

  it('handles message-less objects, null-proto objects, and primitives', () => {
    for (const weird of [{ code: 500 }, Object.create(null), 42, null, undefined, '']) {
      const out = friendlyError(weird);
      expect(out).not.toContain('[object Object]');
      expect(out.length).toBeGreaterThan(0);
    }
  });

  it('translates fetch network failures out of browser-speak', () => {
    expect(friendlyError(new TypeError('Failed to fetch'))).toMatch(/connection/i);
  });

  it('maps the sequencer-unreachable code', () => {
    expect(friendlyError(new ApiError('SEQUENCER_UNREACHABLE', 'could not reach…', 0))).toMatch(
      /can't reach the sequencer/i,
    );
  });

  it('keeps the RECIPIENT_UNKNOWN guidance', () => {
    expect(friendlyError(new ApiError('RECIPIENT_UNKNOWN', 'x', 404))).toMatch(/deposit first/);
  });
});
