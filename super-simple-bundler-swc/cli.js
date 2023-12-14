#!/usr/bin/env node
// @flow
'use strict';

require('babel-register');
const path = require('path');
const meow = require('meow');
const Bundler = require('./src/Bundler').default;

const cli = meow({
  help: `
    Usage
      $ super-simple-bundler-swc <entryFile>
  `,
});

let entryFilePath = path.join(process.cwd(), cli.input[0]);

let bundler = new Bundler(entryFilePath);

bundler.bundle();
