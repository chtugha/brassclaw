fn main() {
    // The proc macro embeds the files it sees at compilation. Track the
    // directory too: adding a migration must invalidate a previously built
    // binary, even when none of its existing include_str! inputs changed.
    println!("cargo::rerun-if-changed=migrations");
}
