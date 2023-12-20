use std::path::PathBuf;

use rsbundler::Bundler;

fn main() {
    let mut bundler = Bundler::new(PathBuf::from("./src/index.js"));
    bundler.bundle();
}
