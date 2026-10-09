/** Assert that a fixture or verified result supplies an optional SDK field. */
export function required<T>(value: T | undefined | null): T {
  if (value == null) throw new Error('Expected fixture/result field to be present');
  return value;
}
