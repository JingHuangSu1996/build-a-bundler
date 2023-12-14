import fs from 'fs';
import path from 'path';
import { promisify } from 'util';
import chalk from 'chalk';
import { parse, transform } from '@swc/core';
import PQueue from 'p-queue';
import mkdirpCb from 'mkdirp';
import resolveFrom from 'resolve-from';
import prettier from 'prettier';

const readFile = promisify(fs.readFile);
const mkdirp = promisify(mkdirpCb);

let currId = 0;

export default class Bundler {
  constructor(entryFilePath) {
    this.entryFilePath = entryFilePath;
    this.processQueue = new PQueue();
    this.assetGraph = new Map();
  }

  async bundle() {
    await this.processAssets();
    await this.packageAssetsIntoBundles();
    console.log(chalk.green('Done!'));
  }

  processAssets() {
    this.createAsset(this.entryFilePath);

    return this.processQueue.onIdle();
  }

  addToProcessQueue(asset) {
    this.processQueue.add(() => this.processAsset(asset));
  }

  createAsset(filePath) {
    let id = currId++;
    let asset = { id, filePath };
    this.assetGraph.set(filePath, asset);
    this.addToProcessQueue(asset);
    return asset;
  }

  async processAsset(asset) {
    let { filePath } = asset;
    let fileContents = await readFile(filePath, 'utf8');

    let ast = await parse(fileContents, {
      syntax: 'ecmascript', // "ecmascript" | "typescript"
      comments: false,
      script: true,

      // Defaults to es3
      target: 'es3',
    });

    let dependencyRequests = [];

    for (let node of ast.body) {
      if (node.type === 'ImportDeclaration') {
        dependencyRequests.push(node.source.value);
      }
    }

    let dependencyMap = new Map();
    dependencyRequests.forEach((moduleRequest) => {
      let srcDir = path.dirname(filePath);
      let dependencyPath = resolveFrom(srcDir, moduleRequest);

      let dependencyAsset = this.assetGraph.get(dependencyPath) || this.createAsset(dependencyPath);
      dependencyMap.set(moduleRequest, dependencyAsset);
    });

    let { code } = await transform(ast, {
      jsc: {},
      module: {
        type: 'commonjs',
        noInterop: true,
      },
    });

    asset.code = code;
    asset.dependencyMap = dependencyMap;
  }

  async packageAssetsIntoBundles() {
    let modules = '';

    this.assetGraph.forEach((asset) => {
      let mapping = {};
      console.log(asset);
      asset.dependencyMap && asset.dependencyMap.forEach((depAsset, key) => (mapping[key] = depAsset.id));
      modules += `${asset.id}: [
        function (require, module, exports) {
          ${asset.code}
        },
        ${JSON.stringify(mapping)},
      ],`;
    });

    // wrapper code taken from https://github.com/ronami/minipack/blob/master/src/minipack.js
    const result = `
      (function(modules) {
        function require(id) {
          const [fn, mapping] = modules[id];

          function localRequire(name) {
            return require(mapping[name]);
          }

          const module = { exports : {} };

          fn(localRequire, module, module.exports);

          return module.exports;
        }

        require(0);
      })({${modules}})
    `;

    await mkdirp('dist');

    const formatted = await prettier.format(result, {
      semi: true,
      trailingComma: 'all',
      singleQuote: true,
      printWidth: 120,
      parser: 'babel',
    });

    fs.writeFileSync('dist/bundle.js', formatted);

    return result;
  }
}
