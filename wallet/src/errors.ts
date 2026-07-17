// Turn sequencer error codes into guidance a user can act on. The raw codes
// (RECIPIENT_UNKNOWN, INSUFFICIENT_BALANCE, …) are precise but unfriendly.
import { ApiError } from './api/sequencer';
import { stroopsToXlm } from './format';

/** Parse a trailing integer out of a sequencer message like "…: available 20000000". */
function trailingAmount(message: string): bigint | null {
  const m = message.match(/(\d+)\s*$/);
  return m ? BigInt(m[1]) : null;
}

export function friendlyError(error: unknown): string {
  if (error instanceof ApiError) {
    switch (error.code) {
      case 'RECIPIENT_UNKNOWN':
        return "This recipient isn't on the rollup yet. A new account has to make one deposit before it can receive payments — ask them to deposit first.";
      case 'INSUFFICIENT_BALANCE': {
        const avail = trailingAmount(error.message);
        return avail != null
          ? `Not enough balance. You can send up to ${stroopsToXlm(avail)} XLM (pending transfers are already reserved).`
          : 'Not enough balance for this transfer.';
      }
      case 'NONCE_MISMATCH':
        return 'Your account just changed — give it a moment to sync, then try again.';
      case 'BAD_SIGNATURE':
        return 'Signature check failed. Try again, or re-import your key.';
      case 'BAD_FIELD':
        return `Invalid input: ${error.message.replace(/^BAD_FIELD:\s*/, '')}`;
      case 'RATE_LIMITED':
        return 'The sequencer is busy right now — wait a minute and try again.';
      case 'SEQUENCER_UNREACHABLE':
        return "Can't reach the sequencer. Check your connection and try again in a moment.";
      case 'BAD_RESPONSE':
        return 'The sequencer returned an unexpected response. Try again in a moment.';
      default:
        return error.message;
    }
  }
  return describeThrown(error);
}

/** Best-effort human text for an arbitrary thrown value. Guarantees we never
 *  render "[object Object]" (e.g. Freighter and DOM errors are objects). */
function describeThrown(error: unknown): string {
  if (typeof error === 'string' && error) return error;
  if (error instanceof Error) {
    // fetch() failures are TypeErrors with browser-speak like "Failed to fetch".
    if (error instanceof TypeError && /fetch|network/i.test(error.message)) {
      return 'Network error — check your connection and try again.';
    }
    return error.message || error.name;
  }
  if (error && typeof error === 'object') {
    const m = (error as { message?: unknown }).message;
    if (typeof m === 'string' && m) return m;
    try {
      const json = JSON.stringify(error);
      if (json && json !== '{}') return json.length > 200 ? `${json.slice(0, 200)}…` : json;
    } catch {
      /* circular — fall through */
    }
    return 'Something went wrong (unrecognized error).';
  }
  return String(error) || 'Something went wrong (empty error).';
}
