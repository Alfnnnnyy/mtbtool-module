import { describe, it, expect } from 'vitest';
import { formatHexByte, parseHexStringToBytes, formatBytesToHex, toggleSetItem } from './helpers';
import { shellEscapeArg, buildCliCommand } from './bridge';

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

describe('bridge escaping', () => {
  it('escapes shell args with single quotes', () => {
    expect(shellEscapeArg('hello')).toBe("'hello'");
    expect(shellEscapeArg("it's test")).toBe("'it'\"'\"'s test'");
  });

  it('builds CLI command with escaped flags', () => {
    const cmd = buildCliCommand('nv write', { path: '/nv/test', hex: '00' });
    expect(cmd).toBe("nv write --path '/nv/test' --hex '00'");
  });
});
