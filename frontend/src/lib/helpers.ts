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
