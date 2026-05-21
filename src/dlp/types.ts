export type Format = 'pdf' | 'png' | 'jpeg' | 'svg' | 'docx';

export interface PayloadValue {
  category: string;
  value: string;
}

export interface Technique {
  id: string;
  format: Format;
  label: string;
  embed: (clean: Buffer, payload: PayloadValue[]) => Promise<Buffer> | Buffer;
}

export interface ManifestEntry {
  format: Format;
  technique: string;
  clean: string;
  dirty: string;
  values: PayloadValue[];
}

export interface GenerateOptions {
  types: Format[];
  count: number;
  out: string;
  techniques: 'all' | string[];
  seed?: number;
}
