//! Write the sample campaign to a folder: `cargo run -p cm_core --example make_sample -- <dir>`
fn main() {
    let dir = std::env::args().nth(1).expect("usage: make_sample <folder>");
    let c = cm_core::sample_campaign();
    cm_core::storage::save(&c, std::path::Path::new(&dir)).expect("save failed");
    println!("wrote {dir}");
}
