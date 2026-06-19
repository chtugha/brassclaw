// V1 - DISABLED - entire main binary depends on deleted V1 modules
// (agent, tools, channels::wasm, channels::web, worker, auth)
// Stub main function to satisfy compiler

fn main() {
    eprintln!("main binary is disabled - V1 code removed");
    eprintln!("Use 'cargo run --bin brassclaw_cli' instead for V2 functionality");
    std::process::exit(1);
}

// Made with Bob
