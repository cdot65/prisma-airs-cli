#!/usr/bin/env node

import { installProcessGuards } from './process-guards.js';
import { buildProgram } from './program.js';

installProcessGuards();
await buildProgram().parseAsync();
