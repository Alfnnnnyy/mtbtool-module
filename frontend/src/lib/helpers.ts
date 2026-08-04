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
  /** exit code of the mtb write command; null when the write never started */
  write_exit?: number | null;
  stage?: string;
  backup_id?: string | null;
  /** observed read-back hex (or null when absent/unreadable) */
  observed_after?: string | null;
  verify_read_error?: string | null;
  rollback_attempted?: boolean;
  rollback_verified?: boolean;
  /** deprecated alias kept for compatibility */
  exit?: number | null;
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
/**
 * Strict three-way write-state classifier. Only EXACT `true`/`false` prove
 * anything: true = write attempted; false = write never reached the modem;
 * anything else (missing, null, wrong type) = unknown — never "nothing
 * written".
 */
export function classifyWriteState(p: NrWritePayload | undefined):
  { kind: 'attempted'; payload: NrWritePayload }
  | { kind: 'not_written'; payload: NrWritePayload }
  | { kind: 'unknown'; reason: string } {
  if (p && p.write_attempted === true) return { kind: 'attempted', payload: p };
  if (p && p.write_attempted === false) return { kind: 'not_written', payload: p };
  return { kind: 'unknown', reason: 'write_attempted missing or malformed' };
}

/**
 * Describe a write outcome from the BACKEND RESPONSE SHAPE, classifying
 * write_attempted FIRST. verified:false is only interpreted inside the
 * "attempted" branch — a pre-write abort (validation/lock/read_before/
 * backup) must never be described as a failed write.
 */
export function describeWriteOutcome(p: NrWritePayload | undefined): string {
  const state = classifyWriteState(p);
  if (state.kind === 'unknown') {
    return `Apply result incomplete — write state is unknown (${state.reason})`;
  }
  if (state.kind === 'not_written') {
    // "nothing written" requires ok:false; anything else is malformed
    if (p!.ok === false) {
      return `Apply failed (nothing written, stage ${p!.stage || '?'}): ${p!.error || 'unknown error'}`;
    }
    return 'Apply result incomplete — write state is unknown (ok:true with write_attempted:false is malformed)';
  }
  // write_attempted === true
  if (p!.verified === true) {
    if (p!.ok === true) {
      return `NR mode applied and verified (backup ${p!.backup_id || '?'})`;
    }
    return 'Apply result incomplete — write state is unknown (verified:true with ok:false is malformed)';
  }
  if (p!.verified === false) {
    if (p!.rollback_attempted === true) {
      return `NR mode write attempted (stage ${p!.stage || 'rollback'}): ${p!.error || 'verification failed'}` +
        ` — rollback attempted, verified ${p!.rollback_verified === true ? 'yes' : 'NO'}` +
        (p!.backup_id ? ` (backup ${p!.backup_id})` : '') +
        (p!.verify_read_error ? `; verify read error: ${p!.verify_read_error}` : `; observed after: ${p!.observed_after ?? 'absent'}`);
    }
    // write reached the modem but the read-back did not confirm the target
    // (stage write/verify without rollback): state unchanged or verification
    // failed — never "nothing written".
    return `NR mode write attempted (stage ${p!.stage || '?'}): ${p!.error || 'write command failed'}` +
      ` — read-back shows ${p!.observed_after ?? 'absent'}; state unchanged or verification failed, no rollback needed` +
      (p!.verify_read_error ? `; verify read error: ${p!.verify_read_error}` : '');
  }
  return 'Apply result incomplete — write state is unknown (verified field missing)';
}

export async function runNrModeApply(
  write: () => Promise<unknown>,
  reRead: () => Promise<NrReReadResult>,
): Promise<string> {
  let base: string;
  try {
    base = describeWriteOutcome((await write()) as NrWritePayload);
  } catch (e: unknown) {
    const err = e as { payload?: unknown; message?: string };
    const payload = err && err.payload;
    if (payload && typeof payload === 'object') {
      base = describeWriteOutcome(payload as NrWritePayload);
    } else {
      // transport/exec failure after dispatch — never claim a verdict
      base = `Apply result unavailable — write state is unknown (${err?.message || 'transport error'})`;
    }
  }

  const reReadOutcome = await reRead();
  return composeNrModeResultMsg(base, {
    ok: reReadOutcome.ok,
    value: reReadOutcome.value,
    byte: reReadOutcome.byte,
  });
}
