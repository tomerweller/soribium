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
      default:
        return error.message;
    }
  }
  if (error instanceof Error) return error.message;
  return String(error);
}
