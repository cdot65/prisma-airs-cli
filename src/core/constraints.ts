import type { CustomTopic } from './types.js';

export interface ValidationError {
  field: string;
  message: string;
}

export const MAX_NAME_LENGTH = 100;
export const MAX_DESCRIPTION_LENGTH = 250;
export const MAX_EXAMPLE_LENGTH = 250;
export const MAX_EXAMPLES = 5;
export const MIN_EXAMPLES = 2;
export const MAX_COMBINED_LENGTH = 1000;

/** UTF-8 byte length — AIRS API enforces limits in bytes, not JS characters. */
export function byteLen(s: string): number {
  return Buffer.byteLength(s, 'utf8');
}

export function validateName(name: string): ValidationError[] {
  const errors: ValidationError[] = [];
  if (!name || name.length === 0) {
    errors.push({ field: 'name', message: 'Name is required' });
  } else if (byteLen(name) > MAX_NAME_LENGTH) {
    errors.push({ field: 'name', message: `Name must be at most ${MAX_NAME_LENGTH} bytes` });
  }
  return errors;
}

export function validateDescription(description: string): ValidationError[] {
  const errors: ValidationError[] = [];
  if (!description || description.length === 0) {
    errors.push({ field: 'description', message: 'Description is required' });
  } else if (byteLen(description) > MAX_DESCRIPTION_LENGTH) {
    errors.push({
      field: 'description',
      message: `Description must be at most ${MAX_DESCRIPTION_LENGTH} bytes`,
    });
  }
  return errors;
}

export function validateExample(example: string, index: number): ValidationError[] {
  const errors: ValidationError[] = [];
  if (!example || example.length === 0) {
    errors.push({ field: `examples[${index}]`, message: `Example ${index} is required` });
  } else if (byteLen(example) > MAX_EXAMPLE_LENGTH) {
    errors.push({
      field: `examples[${index}]`,
      message: `Example ${index} must be at most ${MAX_EXAMPLE_LENGTH} bytes`,
    });
  }
  return errors;
}

export function validateExamples(examples: string[]): ValidationError[] {
  const errors: ValidationError[] = [];
  if (examples.length < MIN_EXAMPLES) {
    errors.push({
      field: 'examples',
      message: `at least ${MIN_EXAMPLES} examples required`,
    });
  }
  if (examples.length > MAX_EXAMPLES) {
    errors.push({
      field: 'examples',
      message: `At most ${MAX_EXAMPLES} examples allowed`,
    });
  }
  for (let i = 0; i < examples.length; i++) {
    errors.push(...validateExample(examples[i], i));
  }
  return errors;
}

export function validateTopic(topic: CustomTopic): ValidationError[] {
  const errors: ValidationError[] = [];
  errors.push(...validateName(topic.name));
  errors.push(...validateDescription(topic.description));
  errors.push(...validateExamples(topic.examples));

  const combined =
    byteLen(topic.name) +
    byteLen(topic.description) +
    topic.examples.reduce((sum, ex) => sum + byteLen(ex), 0);

  if (combined > MAX_COMBINED_LENGTH) {
    errors.push({
      field: 'topic',
      message: `Combined length (${combined}) exceeds ${MAX_COMBINED_LENGTH} bytes`,
    });
  }

  return errors;
}
