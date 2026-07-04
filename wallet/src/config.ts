// App-wide configuration derived from the Vite environment. All server state
// flows through the sequencer at SEQUENCER_URL; on-chain reads and the deposit
// flow use the Stellar testnet parameters returned by GET /params.

export const SEQUENCER_URL: string =
  (import.meta.env.VITE_SEQUENCER_URL as string | undefined) ?? '/api';

/** stellar.expert testnet explorer base. */
export const EXPLORER = 'https://stellar.expert/explorer/testnet';

/** Friendbot funds new testnet accounts. */
export const FRIENDBOT = 'https://friendbot.stellar.org';

/** react-query poll cadence for live server state. */
export const POLL_MS = 4000;

export const contractUrl = (contractId: string) => `${EXPLORER}/contract/${contractId}`;
export const txUrl = (hash: string) => `${EXPLORER}/tx/${hash}`;
