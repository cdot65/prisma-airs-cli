import {
  AISecSDKException,
  buildDottedObject,
  ErrorType,
  type GatewayJsonObject,
  GatewayJsonObjectSchema,
  type GatewayJsonValue,
  setDottedValue,
} from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { CliUsageError } from '../../renderer/index.js';
import { readGatewayRequest } from './shared.js';

interface RequestSchema<T> {
  parse(value: unknown): T;
}

export interface NamedRequestField {
  /** Commander camelCase option key. */
  option: string;
  /** SDK request path, including dotted/bracketed nested segments. */
  path: string;
  parse?: (value: unknown) => GatewayJsonValue;
}

interface StructuredRequestOptions extends Record<string, unknown> {
  file?: string;
  set?: string[];
  setString?: string[];
}

/** Commander accumulator for a repeatable option. */
export function collectOption(value: string, previous: string[] = []): string[] {
  return [...previous, value];
}

/** Add the common structured request source options to one mutation command. */
export function addStructuredInputOptions(command: Command): Command {
  return command
    .option('--file <path>', 'Advanced JSON/YAML request base; named flags override it')
    .option(
      '--set <path=value>',
      'Set a typed JSON value at a request path (repeatable)',
      collectOption,
    )
    .option(
      '--set-string <path=value>',
      'Set a literal string at a request path (repeatable)',
      collectOption,
    );
}

export function parseBooleanOption(value: unknown): boolean {
  if (value === true || value === false) return value;
  if (value === 'true') return true;
  if (value === 'false') return false;
  throw new CliUsageError('Expected true or false');
}

export function parseIntegerOption(value: unknown): number {
  const text = String(value);
  if (!/^-?(0|[1-9]\d*)$/.test(text)) throw new CliUsageError('Expected an integer');
  const parsed = Number(text);
  if (!Number.isSafeInteger(parsed)) throw new CliUsageError('Expected a safe integer');
  return parsed;
}

export function parseDateOption(value: unknown): Date {
  const parsed = new Date(String(value));
  if (Number.isNaN(parsed.getTime())) throw new CliUsageError('Expected an ISO-8601 timestamp');
  return parsed;
}

export function parseCsvOption(value: unknown): string[] {
  const values = String(value)
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
  if (values.length === 0) throw new CliUsageError('Expected a non-empty comma-separated list');
  return values;
}

export function parseJsonOption(value: unknown): GatewayJsonValue {
  try {
    return JSON.parse(String(value)) as GatewayJsonValue;
  } catch {
    throw new CliUsageError('Expected valid JSON');
  }
}

function repeatableValues(value: unknown): string[] {
  if (Array.isArray(value)) return value.map(String);
  return [String(value)];
}

function splitKeyValue(raw: string, label: string): [string, string] {
  const separator = raw.indexOf('=');
  if (separator <= 0) throw new CliUsageError(`Expected ${label}=value`);
  return [raw.slice(0, separator), raw.slice(separator + 1)];
}

export function parseBooleanBindingsOption(value: unknown): GatewayJsonValue {
  return repeatableValues(value).map((raw) => {
    const [id, enabled] = splitKeyValue(raw, 'id');
    return { id, enabled: parseBooleanOption(enabled) };
  });
}

export function parseModelBindingsOption(value: unknown): GatewayJsonValue {
  return repeatableValues(value).map((raw) => {
    const [slug, enabled] = splitKeyValue(raw, 'slug');
    return { slug, enabled: parseBooleanOption(enabled) };
  });
}

export function parseCapabilityBindingsOption(value: unknown): GatewayJsonValue {
  return repeatableValues(value).map((raw) => {
    const [identity, enabled] = splitKeyValue(raw, 'type:name');
    const separator = identity.indexOf(':');
    if (separator <= 0 || separator === identity.length - 1) {
      throw new CliUsageError('Expected type:name=enabled');
    }
    return {
      type: identity.slice(0, separator),
      name: identity.slice(separator + 1),
      enabled: parseBooleanOption(enabled),
    };
  });
}

export function parseStringMapOption(value: unknown): GatewayJsonValue {
  const result: Record<string, string> = {};
  for (const raw of repeatableValues(value)) {
    const [key, item] = splitKeyValue(raw, 'key');
    if (['__proto__', 'constructor', 'prototype'].includes(key)) {
      throw new CliUsageError(`Unsafe key: ${key}`);
    }
    if (Object.hasOwn(result, key)) throw new CliUsageError(`Duplicate key: ${key}`);
    result[key] = item;
  }
  return result;
}

function parseAssignment(raw: string, forceString: boolean): [string, GatewayJsonValue] {
  const separator = raw.indexOf('=');
  if (separator <= 0) throw new CliUsageError('Expected path=value');
  const path = raw.slice(0, separator);
  const text = raw.slice(separator + 1);
  if (forceString) return [path, text];
  try {
    return [path, JSON.parse(text) as GatewayJsonValue];
  } catch {
    return [path, text];
  }
}

function schemaMessage(error: unknown): string {
  const issues = (error as { issues?: Array<{ message?: string; path?: PropertyKey[] }> })?.issues;
  if (!Array.isArray(issues) || issues.length === 0)
    return 'request body does not match its schema';
  return issues
    .map((issue) => {
      const path = issue.path?.length ? issue.path.map(String).join('.') : '<root>';
      return `${path}: ${issue.message ?? 'invalid value'}`;
    })
    .join('; ');
}

function optionName(option: string): string {
  return `--${option.replace(/[A-Z]/g, (character) => `-${character.toLowerCase()}`)}`;
}

function asUsageError(error: unknown, prefix = 'Invalid AI Gateway request'): CliUsageError {
  if (error instanceof CliUsageError) return error;
  if (
    error instanceof AISecSDKException &&
    error.errorType === ErrorType.USER_REQUEST_PAYLOAD_ERROR
  ) {
    return new CliUsageError(`${prefix}: ${error.message}`);
  }
  return new CliUsageError(`${prefix}: ${schemaMessage(error)}`);
}

/**
 * Merge an optional request file, named CLI fields, and repeatable dotted assignments.
 * Precedence is file < named flags < --set/--set-string. The final SDK schema is parsed
 * before a client is constructed, so malformed writes never reach OAuth or the network.
 */
export async function buildStructuredRequest<T>(
  options: StructuredRequestOptions,
  schema: RequestSchema<T>,
  fields: readonly NamedRequestField[] = [],
): Promise<T> {
  try {
    let body: GatewayJsonObject = {};
    if (options.file) {
      body = GatewayJsonObjectSchema.parse(await readGatewayRequest(options.file));
    }

    for (const field of fields) {
      const raw = options[field.option];
      if (raw === undefined || (Array.isArray(raw) && raw.length === 0)) continue;
      let value: GatewayJsonValue;
      try {
        value = field.parse ? field.parse(raw) : (raw as GatewayJsonValue);
      } catch (error) {
        if (error instanceof CliUsageError) {
          throw new CliUsageError(`Invalid ${optionName(field.option)}: ${error.message}`);
        }
        throw error;
      }
      body = setDottedValue(body, field.path, value);
    }
    const assignments = [
      ...(options.set ?? []).map((raw) => {
        try {
          const [path, value] = parseAssignment(raw, false);
          return { path, value };
        } catch (error) {
          if (error instanceof CliUsageError) {
            throw new CliUsageError(`Invalid --set '${raw}': ${error.message}`);
          }
          throw error;
        }
      }),
      ...(options.setString ?? []).map((raw) => {
        try {
          const [path, value] = parseAssignment(raw, true);
          return { path, value };
        } catch (error) {
          if (error instanceof CliUsageError) {
            throw new CliUsageError(`Invalid --set-string '${raw}': ${error.message}`);
          }
          throw error;
        }
      }),
    ];
    // Validate the assignment set as a whole so duplicate paths and scalar/container
    // collisions are errors instead of becoming order-dependent last-write-wins behavior.
    buildDottedObject(assignments);
    for (const { path, value } of assignments) body = setDottedValue(body, path, value);

    return schema.parse(body);
  } catch (error) {
    throw asUsageError(error);
  }
}
