#[warn(unused_imports)]
use std::{
    path::PathBuf,
    rc::Rc,
    cell::RefCell,
    sync::Arc,
};

use std::collections::HashMap;
use swc_ecma_parser::parse_file_as_module;
use swc_ecma_ast::{EsVersion, ImportDecl, Module, Program};

use swc:: {
    config:: {Config, JscConfig, Options},
    Compiler,
};

use swc_common::{
    errors::{ColorConfig, Handler},
    Globals, SourceMap, GLOBALS,
};

#[derive(Debug)]
struct Asset {
    id: u64,
    path: PathBuf,
    code: RefCell<String>,
    dependencies: RefCell<HashMap<PathBuf, Rc<Asset>>>,
}

#[derive(Debug, Default)]
struct ProcessQueue {
    queue: Vec<Rc<Asset>>,
}

pub struct Bundler {
    cm: Arc<SourceMap>,
    compiler: Compiler,
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

// rust impl
impl Bundler {
    pub fn new(entry: PathBuf) -> Self {
        let cm = Arc::new(SourceMap::default());
        let compiler = Compiler::new(cm.clone());
        Self {
            cm,
            compiler,
            entry,
            asset_graph: Default::default(),
            current_id: Default::default(),
            process_queue: Default::default(),
        }
    }
    pub fn bundle(&mut self) {
        self.process_assets();
    }

    fn process_assets(&mut self) {
        // create asset
        self.create_asset(self.entry.clone());

        while let Some(asset) = self.process_queue.queue.pop() {
            self.process_asset(asset)
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
        );

        println!("ast: {:?}", ast); 
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