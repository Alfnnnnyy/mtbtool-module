import { describe, it, expect } from 'vitest';
import { formatHexByte, parseHexStringToBytes, formatBytesToHex, toggleSetItem, composeNrModeResultMsg } from './helpers';
import { encodeBase64Url } from './bridge';

describe('helpers', () => {
  it('formats hex byte correctly', () => {
    expect(formatHexByte(0)).toBe('00');
    expect(formatHexByte(15)).toBe('0f');
    expect(formatHexByte(255)).toBe('ff');
  });

  it('parses hex string to bytes and formats back', () => {
    const hex = '0103070f';
    const bytes = parseHexStringToBytes(hex);
    expect(bytes).toEqual([1, 3, 7, 15]);
    expect(formatBytesToHex(bytes)).toBe(hex);
  });

  it('toggles set item correctly', () => {
    const s1 = new Set<number>([1, 3]);
    const s2 = toggleSetItem(s1, 7);
    expect(Array.from(s2)).toEqual([1, 3, 7]);
    const s3 = toggleSetItem(s2, 3);
    expect(Array.from(s3)).toEqual([1, 7]);
  });
});

describe('encodeBase64Url', () => {
  it('handles empty input', () => {
    expect(encodeBase64Url('')).toBe('');
    expect(encodeBase64Url(new Uint8Array([]))).toBe('');
  });

  it('encodes plain ascii string without padding or + / characters', () => {
    const encoded = encodeBase64Url('hello world');
    expect(encoded).toBe('aGVsbG8gd29ybGQ');
    expect(encoded).not.toContain('=');
    expect(encoded).not.toContain('+');
    expect(encoded).not.toContain('/');
  });

  it('encodes binary data replacing + with - and / with _ and trimming padding =', () => {
    // Uint8Array([0, 1, 2, 255, 254]) -> base64 AAEC//4= -> base64url AAEC--4
    const bin = new Uint8Array([0, 1, 2, 255, 254]);
    const encoded = encodeBase64Url(bin);
    expect(encoded).toBe('AAEC__4');
    expect(encoded).not.toContain('=');
    expect(encoded).not.toContain('+');
  });

  it('encodes unicode strings correctly', () => {
    const str = 'POCO F6 ⚡';
    const encoded = encodeBase64Url(str);
    expect(encoded).toBe('UE9DTyBGNiDimqE');
    expect(encoded).not.toContain('=');
  });

  it('builds valid rpc payload json structure before encoding', () => {
    const payload = { method: 'nv.write', params: { path: '/nv/item_files/modem/test', hex: '0102', slot: 0 } };
    const jsonStr = JSON.stringify(payload);
    const b64 = encodeBase64Url(jsonStr);
    expect(b64).not.toContain('=');
    expect(b64).not.toContain('+');
    expect(b64).not.toContain('/');
    
    // Verify decoded json matches original
    const decodedJson = Buffer.from(b64.replace(/-/g, '+').replace(/_/g, '/'), 'base64').toString('utf-8');
    expect(JSON.parse(decodedJson)).toEqual(payload);
  });
});

describe('composeNrModeResultMsg (NR failure-path regression)', () => {
  const applyMsg = 'Apply attempted: verification failed — rollback attempted yes, verified yes (backup 123_1_x)';

  it('preserves the apply/rollback message and appends confirmed state on a good re-read', () => {
    const out = composeNrModeResultMsg(applyMsg, { ok: true, value: 2, byte: '02' });
    expect(out).toContain(applyMsg);
    expect(out).toContain('confirmed current: SA Only (byte 0x02)');
  });

  it('keeps the apply message and reports unknown state when the re-read fails', () => {
    const out = composeNrModeResultMsg(applyMsg, { ok: false, value: null, byte: '' });
    expect(out).toContain(applyMsg);
    expect(out).toContain('live re-read failed — current modem state is unknown');
    expect(out).not.toContain('confirmed current');
  });

  it('never uses a stale cached value as confirmation', () => {
    // value=1 would be a stale cache if the re-read actually failed
    const out = composeNrModeResultMsg(applyMsg, { ok: false, value: 1, byte: '01' });
    expect(out).not.toContain('confirmed current');
    expect(out).toContain('unknown');
  });

  it('rejects out-of-range values even when ok', () => {
    const out = composeNrModeResultMsg(applyMsg, { ok: true, value: 9, byte: '09' });
    expect(out).not.toContain('confirmed current');
    expect(out).toContain('unknown');
  });
});
