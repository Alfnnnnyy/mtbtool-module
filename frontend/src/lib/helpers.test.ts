import { describe, it, expect } from 'vitest';
import { formatHexByte, parseHexStringToBytes, formatBytesToHex, toggleSetItem, composeNrModeResultMsg, runNrModeApply, classifyWriteState, describeWriteOutcome } from './helpers';
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

describe('runNrModeApply (production orchestrator, injected mocks)', () => {
  it('transport error without payload must not claim "nothing written" and must survive a failing re-read', async () => {
    const write = () => Promise.reject(new Error('exec bridge failed'));
    const reRead = () => Promise.resolve({ ok: false, value: null, byte: '', error: 'nv.read failed' });
    const msg = await runNrModeApply(write, reRead);
    expect(msg).not.toContain('nothing written');
    expect(msg).toContain('write state is unknown');
    expect(msg).toContain('live re-read failed — current modem state is unknown');
    expect(msg).toContain('exec bridge failed');
  });

  it('transport error + successful re-read: write unknown, observed byte reported', async () => {
    const write = () => Promise.reject(new Error('stdout lost'));
    const reRead = () => Promise.resolve({ ok: true, value: 2, byte: '02', error: '' });
    const msg = await runNrModeApply(write, reRead);
    expect(msg).not.toContain('nothing written');
    expect(msg).toContain('write state is unknown');
    expect(msg).toContain('confirmed current: SA Only (byte 0x02)');
  });

  it('write_attempted:false payload explicitly allows "nothing written"', async () => {
    const write = () => Promise.resolve({ ok: false, error: 'Backup failed, write aborted: x', write_attempted: false, stage: 'backup' });
    const reRead = () => Promise.resolve({ ok: true, value: 1, byte: '01', error: '' });
    const msg = await runNrModeApply(write, reRead);
    expect(msg).toContain('nothing written');
    expect(msg).toContain('confirmed current: NSA Only (byte 0x01)');
  });

  it('write_attempted:true payload reports attempt + rollback, never "nothing written"', async () => {
    const write = () => Promise.resolve({
      ok: false, error: 'verification failed', verified: false, write_attempted: true, stage: 'rollback',
      backup_id: 'b1', rollback_attempted: true, rollback_verified: false, observed_after: '00',
    });
    const reRead = () => Promise.resolve({ ok: false, value: null, byte: '', error: '' });
    const msg = await runNrModeApply(write, reRead);
    expect(msg).not.toContain('nothing written');
    expect(msg).toContain('rollback attempted, verified NO');
    expect(msg).toContain('(backup b1)');
    expect(msg).toContain('unknown');
  });
});

describe('three-way write-state classifier (missing write_attempted)', () => {
  it('resolved {ok:false} without write_attempted must say unknown, not nothing written', async () => {
    const write = () => Promise.resolve({ ok: false, error: 'x' });
    const reRead = () => Promise.resolve({ ok: true, value: 1, byte: '01', error: '' });
    const msg = await runNrModeApply(write, reRead);
    expect(msg).not.toContain('nothing written');
    expect(msg).toContain('write state is unknown');
    expect(msg).toContain('confirmed current');
  });

  it('rejected payload {ok:false} without write_attempted must say unknown, not nothing written', async () => {
    const err = new Error('transport');
    (err as unknown as { payload?: unknown }).payload = { ok: false, error: 'x' };
    const write = () => Promise.reject(err);
    const reRead = () => Promise.resolve({ ok: false, value: null, byte: '', error: 'read fail' });
    const msg = await runNrModeApply(write, reRead);
    expect(msg).not.toContain('nothing written');
    expect(msg).toContain('write state is unknown');
    expect(msg).toContain('live re-read failed');
  });

  it('null/wrong-typed write_attempted also classifies unknown', () => {
    expect(classifyWriteState({ ok: false, error: 'x', write_attempted: null as unknown as boolean }).kind).toBe('unknown');
    expect(classifyWriteState({ ok: false, error: 'x', write_attempted: 'yes' as unknown as boolean }).kind).toBe('unknown');
    expect(classifyWriteState({ ok: false, error: 'x' }).kind).toBe('unknown');
    expect(classifyWriteState({ ok: false, error: 'x', write_attempted: true }).kind).toBe('attempted');
    expect(classifyWriteState({ ok: false, error: 'x', write_attempted: false }).kind).toBe('not_written');
  });
});

describe('branch-order regression: classify BEFORE verified (exact backend shapes)', () => {
  const okRead = () => Promise.resolve({ ok: true, value: 1, byte: '01', error: '' });

  it('A: {ok:false,verified:false,write_attempted:false,stage:validation} -> nothing written, never "NR mode written"', async () => {
    const write = () => Promise.resolve({ ok: false, verified: false, write_attempted: false, stage: 'validation', error: 'bad path' });
    const msg = await runNrModeApply(write, okRead);
    expect(msg).toContain('nothing written');
    expect(msg).not.toContain('NR mode written');
    expect(msg).not.toContain('read-back verification FAILED');
    expect(msg).toContain('confirmed current');
  });

  it('B: {ok:false,verified:false} missing write_attempted -> write state unknown', async () => {
    const write = () => Promise.resolve({ ok: false, verified: false });
    const msg = await runNrModeApply(write, okRead);
    expect(msg).toContain('write state is unknown');
    expect(msg).not.toContain('nothing written');
    expect(msg).not.toContain('NR mode written');
  });

  it('C: {ok:false,verified:false,write_attempted:true,stage:write,rollback_attempted:false} -> attempted, never nothing written', async () => {
    const write = () => Promise.resolve({ ok: false, verified: false, write_attempted: true, stage: 'write', rollback_attempted: false, observed_after: 'aabb' });
    const msg = await runNrModeApply(write, okRead);
    expect(msg).not.toContain('nothing written');
    expect(msg).toContain('write attempted');
    expect(msg).toContain('read-back shows aabb');
  });

  it('D: rejected payload versions of A/B/C behave identically', async () => {
    const rejectWith = (payload: unknown) => {
      const err = new Error('transport');
      (err as unknown as { payload?: unknown }).payload = payload;
      return Promise.reject(err);
    };
    const a = await runNrModeApply(() => rejectWith({ ok: false, verified: false, write_attempted: false, stage: 'validation', error: 'x' }), okRead);
    expect(a).toContain('nothing written');
    expect(a).not.toContain('NR mode written');
    const b = await runNrModeApply(() => rejectWith({ ok: false, verified: false }), okRead);
    expect(b).toContain('write state is unknown');
    expect(b).not.toContain('nothing written');
    const c = await runNrModeApply(() => rejectWith({ ok: false, verified: false, write_attempted: true, stage: 'write', rollback_attempted: false }), okRead);
    expect(c).not.toContain('nothing written');
    expect(c).toContain('write attempted');
  });

  it('ok:true + verified:true with write_attempted:false is malformed -> unknown', () => {
    const msg = describeWriteOutcome({ ok: true, verified: true, write_attempted: false });
    expect(msg).toContain('write state is unknown');
  });
});
