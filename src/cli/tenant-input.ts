import { input, password } from '@inquirer/prompts';

/** Prompts use stderr so stdout remains usable for machine-readable output. */
export async function promptTenantValue(message: string, secret = false): Promise<string> {
  if (!process.stdin.isTTY || !process.stderr.isTTY)
    throw new Error(
      'Interactive setup requires a terminal. Use --config, or --tsg-id and --client-id with --client-secret-stdin. For tenant set, pass a value or use --stdin.',
    );
  const prompt = secret ? password : input;
  return prompt(
    {
      message,
      validate: (value: string) => Boolean(value.trim()) || 'A nonempty value is required',
    },
    { input: process.stdin, output: process.stderr },
  );
}

/** Read one value, allowing a single trailing line ending from secret-manager tools. */
export async function readTenantStdin(): Promise<string> {
  if (process.stdin.isTTY) throw new Error('--stdin requires piped input');
  const chunks: Buffer[] = [];
  let size = 0;
  for await (const chunk of process.stdin) {
    const buffer = Buffer.from(chunk);
    size += buffer.length;
    if (size > 65_536) throw new Error('Configuration input exceeds 64 KiB');
    chunks.push(buffer);
  }
  const value = Buffer.concat(chunks)
    .toString('utf8')
    .replace(/\r?\n$/, '');
  if (!value.trim() || /[\r\n\0]/.test(value))
    throw new Error('Provide one nonempty configuration value on stdin');
  return value;
}
