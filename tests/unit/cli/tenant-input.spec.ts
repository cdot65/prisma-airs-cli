import { input, password } from '@inquirer/prompts';
import { promptTenantValue } from '../../../src/cli/tenant-input.js';

vi.mock('@inquirer/prompts', () => ({ input: vi.fn(), password: vi.fn() }));
const streams = [process.stdin, process.stderr];
const descriptors = streams.map((stream) => Object.getOwnPropertyDescriptor(stream, 'isTTY'));
function terminal(value: boolean) {
  for (const stream of streams)
    Object.defineProperty(stream, 'isTTY', { value, configurable: true });
}
afterEach(() => {
  streams.forEach((stream, index) => {
    const original = descriptors[index];
    if (original) Object.defineProperty(stream, 'isTTY', original);
    else Reflect.deleteProperty(stream, 'isTTY');
  });
  vi.restoreAllMocks();
  vi.clearAllMocks();
});

it('requires a terminal without calling a prompt', async () => {
  terminal(false);
  await expect(promptTenantValue('TSG')).rejects.toThrow('requires a terminal');
  expect(input).not.toHaveBeenCalled();
});

it('uses a hidden password prompt on stderr for credentials', async () => {
  terminal(true);
  vi.mocked(password).mockResolvedValue('FAKE-SECRET');
  expect(await promptTenantValue('Secret', true)).toBe('FAKE-SECRET');
  expect(password).toHaveBeenCalledWith(expect.objectContaining({ message: 'Secret' }), {
    input: process.stdin,
    output: process.stderr,
  });
  expect(input).not.toHaveBeenCalled();
});

it('propagates cancellation without returning a partial value', async () => {
  terminal(true);
  vi.mocked(input).mockRejectedValue(
    Object.assign(new Error('cancelled'), { name: 'ExitPromptError' }),
  );
  await expect(promptTenantValue('TSG')).rejects.toMatchObject({ name: 'ExitPromptError' });
});
