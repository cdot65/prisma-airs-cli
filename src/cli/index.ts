#!/usr/bin/env node

import 'dotenv/config';
import { installProcessGuards } from './process-guards.js';
import { buildProgram } from './program.js';

installProcessGuards();
buildProgram().parse();
