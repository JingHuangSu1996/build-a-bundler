#[warn(unused_imports)]
use std::{
    cell::RefCell,
    collections::HashMap,
    fs::create_dir_all,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use swc::{
    config::{Config, JscConfig, Options},
    Compiler,
};
use swc_atoms::Atom;
use swc_common::{
    errors::{ColorConfig, Handler},
    Globals, SourceMap, GLOBALS,
};
use swc_ecma_ast::{EsVersion, ImportDecl, Module, Program};

use swc_ecma_parser::parse_file_as_module;
use swc_ecma_visit::Visit;

#[derive(Debug)]
struct Asset {
    id: u64,
    path: PathBuf,
    code: RefCell<String>,
    dependencies: RefCell<HashMap<Atom, Rc<Asset>>>,
}

#[derive(Debug, Default)]
struct ProcessQueue {
    queue: Vec<Rc<Asset>>,
}

struct ImportVisitor {
    imports: Vec<Atom>,
}

impl Visit for ImportVisitor {
    fn visit_import_decl(&mut self, node: &ImportDecl) {
        self.imports.push(node.src.value.clone());
    }
}

pub struct Bundler {
    cm: Arc<SourceMap>,
    compiler: Compiler,
    globals: Arc<Globals>,
    entry: PathBuf,
    asset_graph: HashMap<PathBuf, Rc<Asset>>,
    current_id: u64,
    process_queue: ProcessQueue,
}

impl ProcessQueue {
    fn add (&mut self, asset: Rc<Asset>) {
        self.queue.push(asset);
    }
}

const TARGET_ES_VERSION: EsVersion = EsVersion::Es5;

// rust impl
impl Bundler {
    pub fn new(entry: PathBuf) -> Self {
        let cm = Arc::new(SourceMap::default());
        let compiler = Compiler::new(cm.clone());
        Self {
            cm,
            compiler,
            entry,
            globals: Default::default(),
            asset_graph: Default::default(),
            current_id: Default::default(),
            process_queue: Default::default(),
        }
    }
    pub fn bundle(&mut self) {
        GLOBALS.set(&self.globals.clone(), || {
            self.process_assets();
            self.package_assets_into_bundles();
        });
    }

    fn package_assets_into_bundles(&mut self) -> String {
        let mut modules = String::new();

        self.asset_graph.iter().for_each(|(_, asset)| {
           let mut mapping = HashMap::new();

           asset.dependencies.borrow().iter().for_each(|(specifier, dependency)| {
               mapping.insert(specifier.clone(), dependency.id);
           });

           let mapping_json = serde_json::to_string(&mapping).unwrap();

            let code = asset.code.borrow();
            modules.push_str(&format!(
                "{}: [
                function (require, module, exports) {{
                  {}
                }},
                {},
              ],",
                asset.id, code, mapping_json
            ));
        });

        let result = format!(
            "
        (function(modules) {{
          function require(id) {{
            const [fn, mapping] = modules[id];
  
            function localRequire(name) {{
              return require(mapping[name]);
            }}
  
            const module = {{ exports : {{}} }};
  
            fn(localRequire, module, module.exports);
  
            return module.exports;
          }}
  
          require(0);
        }})({{{modules}}})
        ",
        );

        create_dir_all("dist").expect("failed to create dist directory");
        std::fs::write("dist/bundle.js", &result).expect("failed to write bundle.js");

        return result;
    }

    fn process_assets(&mut self) {
        // create asset
        self.create_asset(self.entry.clone());

        while let Some(asset) = self.process_queue.queue.pop() {
            self.process_asset(asset);
        }
    }

    fn process_asset(&mut self, asset: Rc<Asset>) {
        let path = asset.path.clone();
        let fm = self.cm.load_file(&path).expect("failed to load file");
        println!("processing asset: {:?}", path);
        println!("file content: {:?}", fm.src);

        let ast = parse_file_as_module(
            &fm,
            Default::default(),
            EsVersion::Es2020,
            None,
            &mut vec![],
        ).expect("failed to parse file as module");

        let deps = find_imports(&ast);

        let mut dependency_map = HashMap::default();

        for module_request in deps {
            let src_dir = path.parent().unwrap();
            let dependency_path = resolve_from(src_dir, &module_request);

            let dependency_asset = match self.asset_graph.get(&dependency_path) {
                Some(e) => e.clone(),
                None => self.create_asset(dependency_path),
            };

            dependency_map.insert(module_request, dependency_asset.clone());
        }

        let code = self.print_js(ast);

        // println!("ast: {:?}", &deps);
        
        *asset.code.borrow_mut() = code;
        *asset.dependencies.borrow_mut() = dependency_map;
    }

    fn print_js(&mut self, m: Module) -> String {
        let h = Handler::with_tty_emitter(
            ColorConfig::Always,
            true,
            false,
            Some(self.cm.clone()),
        );

        let m = self.compiler.process_js(
            &h, 
            Program::Module(m.clone()),
            &Options {
                config: Config {
                    jsc: JscConfig {
                        target: Some(TARGET_ES_VERSION),
                        ..Default::default()
                    },
                    module: Some(swc::config::ModuleConfig::CommonJs(
                        swc_ecma_transforms::modules::common_js::Config {
                            no_interop: true,
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        ).unwrap();

        m.code
    }

    fn add_to_process_queue(&mut self, asset: Rc<Asset>) {
        // add asset to process queue
        self.process_queue.add(asset);
    }

    fn create_asset(&mut self, path: PathBuf) -> Rc<Asset> {
        let id = self.current_id;
        self.current_id += 1;

        let asset = Rc::new(Asset {
            id, 
            path: path.clone(),
            code: Default::default(),
            dependencies: Default::default(),
        });

        self.asset_graph.insert(path, asset.clone());
        self.add_to_process_queue(asset.clone());
        return asset;
    }

    pub fn print_asset_graph(&self) {
        println!("{:#?}", self.asset_graph);
    }   
}

fn find_imports(ast: &Module) -> Vec<Atom> {
    let mut v = ImportVisitor { imports: vec![] };
    v.visit_module(ast);
    v.imports
}

fn resolve_from(path: &Path, specifier: &str) -> PathBuf {
    let resolver = oxc_resolver::Resolver::new(Default::default());

    let r = resolver.resolve(path, &specifier).unwrap_or_else(|err| {
        panic!(
            "failed to resolve module \"{}\" from \"{}\": {}",
            specifier,
            path.display(),
            err
        )
    });

    r.path().to_path_buf()
}