//! Re-embeds the migrations when a file of `migrations/` is added, edited or removed.

fn main() {
    println!("cargo:rerun-if-changed=../../../migrations");
}
