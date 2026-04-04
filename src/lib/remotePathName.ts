export function validateRemoteLeafName(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return '名称不能为空';
  if (name === '.' || name === '..') return '名称不能为 . 或 ..';
  if (name.includes('/')) return '名称不能包含 /';
  if ([...name].some((ch) => /[\u0000-\u001f\u007f-\u009f]/.test(ch))) {
    return '名称不能包含控制字符';
  }
  return null;
}
