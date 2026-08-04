export function formatHexByte(val: number): string {
  return val.toString(16).padStart(2, '0').toLowerCase();
}

export function parseHexStringToBytes(hexStr: string): number[] {
  const clean = hexStr.replace(/\s+/g, '');
  if (clean.length % 2 !== 0) return [];
  const bytes: number[] = [];
  for (let i = 0; i < clean.length; i += 2) {
    const b = parseInt(clean.substring(i, i + 2), 16);
    if (isNaN(b)) return [];
    bytes.push(b);
  }
  return bytes;
}

export function formatBytesToHex(bytes: number[]): string {
  return bytes.map(formatHexByte).join('');
}

export const ALL_LTE_BANDS: number[] = [
  1, 2, 3, 4, 5, 7, 8, 12, 13, 14, 17, 18, 19, 20, 21,
  25, 26, 28, 29, 30, 32, 34, 38, 39, 40, 41, 42, 43, 46, 48,
  66, 71
];

export const ALL_NR_BANDS: number[] = [
  1, 2, 3, 5, 7, 8, 12, 14, 18, 20, 25, 26, 28, 29, 30,
  34, 38, 39, 40, 41, 46, 48, 50, 51, 53, 65, 66, 70, 71, 74,
  75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 86, 89, 90, 91, 92,
  93, 94, 95, 96, 97, 100, 101, 102, 104,
  257, 258, 260, 261
];

export function toggleSetItem<T>(set: Set<T>, item: T, value?: boolean): Set<T> {
  const next = new Set(set);
  const shouldCheck = value !== undefined ? value : !next.has(item);
  if (shouldCheck) {
    next.add(item);
  } else {
    next.delete(item);
  }
  return next;
}

export const NR_MODE_LABELS = ['SA + NSA (Both)', 'NSA Only', 'SA Only'];

/**
 * Compose the final NR-mode apply message from the apply/rollback result and
 * a LIVE re-read outcome. "confirmed current" is only ever produced from a
 * successful re-read — a stale cached value is never treated as confirmation.
 */
export function composeNrModeResultMsg(
  base: string,
  reRead: { ok: boolean; value: number | null; byte: string },
): string {
  if (reRead.ok && reRead.value !== null && reRead.value >= 0 && reRead.value < NR_MODE_LABELS.length) {
    return `${base} — confirmed current: ${NR_MODE_LABELS[reRead.value]} (byte 0x${reRead.byte})`;
  }
  return `${base} — live re-read failed — current modem state is unknown`;
}

export interface NrReReadResult {
  ok: boolean;
  value: number | null;
  byte: string;
  error: string;
}

export interface NrWritePayload {
  ok?: boolean;
  error?: string;
  verified?: boolean;
  write_attempted?: boolean;
  stage?: string;
  backup_id?: string | null;
  rollback_attempted?: boolean;
  rollback_verified?: boolean;
}

/**
 * Orchestrator for the NR-mode apply + confirm flow (the REAL production
 * path used by Features.svelte; unit-tested with injected mocks).
 *
 * Message rules:
 * - "nothing written" ONLY when the backend explicitly reports
 *   write_attempted === false (validation/lock/read_before/backup stages)
 * - a rejected RPC with NO payload (transport loss, empty stdout) never
 *   claims "nothing written" — it says the write state is unknown
 * - after every outcome a live re-read decides "confirmed current" vs
 *   "current state unknown"
 */
export async function runNrModeApply(
  write: () => Promise<unknown>,
  reRead: () => Promise<NrReReadResult>,
): Promise<string> {
  let base: string;
  try {
    const res = (await write()) as NrWritePayload;
    if (res && res.ok === true && res.verified === true) {
      base = `NR mode applied and verified (backup ${res.backup_id || '?'})`;
    } else if (res && res.verified === false) {
      base = `NR mode written but read-back verification FAILED (backup ${res.backup_id || '?'})` +
        (res.rollback_attempted ? ` — rollback attempted, verified ${res.rollback_verified === true ? 'yes' : 'NO'}` : '');
    } else if (res && res.ok === false) {
      if (res.write_attempted === true) {
        base = `Apply attempted (stage ${res.stage || '?'}): ${res.error || 'verification failed'}` +
          (res.rollback_attempted ? ` — rollback attempted, verified ${res.rollback_verified === true ? 'yes' : 'NO'}` : '') +
          (res.backup_id ? ` (backup ${res.backup_id})` : '');
      } else {
        // backend explicitly says the write never reached the modem write stage
        base = `Apply failed (nothing written, stage ${res.stage || '?'}): ${res.error || 'unknown error'}`;
      }
    } else {
      base = 'Apply result incomplete — write state is unknown';
    }
  } catch (e: unknown) {
    const err = e as { payload?: unknown; message?: string };
    if (err && err.payload && typeof err.payload === 'object') {
      const payload = err.payload as NrWritePayload;
      if (payload.ok === false && payload.write_attempted === true) {
        base = `Apply attempted (stage ${payload.stage || '?'}): ${payload.error || 'verification failed'}` +
          (payload.rollback_attempted ? ` — rollback attempted, verified ${payload.rollback_verified === true ? 'yes' : 'NO'}` : '') +
          (payload.backup_id ? ` (backup ${payload.backup_id})` : '');
      } else if (payload.ok === false) {
        base = `Apply failed (nothing written, stage ${payload.stage || '?'}): ${payload.error || 'unknown error'}`;
      } else {
        base = `Apply result unavailable — write state is unknown (${err.message || 'transport error'})`;
      }
    } else {
      // no payload: transport/exec failure after dispatch — MUST NOT claim
      // the write never started
      base = `Apply result unavailable — write state is unknown (${err.message || 'transport error'})`;
    }
  }

  const reReadOutcome = await reRead();
  return composeNrModeResultMsg(base, {
    ok: reReadOutcome.ok,
    value: reReadOutcome.value,
    byte: reReadOutcome.byte,
  });
}
