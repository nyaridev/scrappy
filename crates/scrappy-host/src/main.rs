//! scrappy-host: the native-messaging host the browser starts for the Scrappy extension. It will relay length-prefixed
//! JSON frames between the extension (stdin/stdout) and the running app over a local socket (ADR-0006, M5).

fn main() {
    eprintln!("scrappy-host isn't wired up yet: the browser bridge arrives in M5.");
    std::process::exit(1);
}
