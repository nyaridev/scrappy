//! scrappy-host: the native-messaging host the browser starts for the Scrappy extension. It relays length-prefixed
//! JSON frames between the extension (stdin/stdout) and the running app's local socket, starting the app when it
//! isn't running (ADR-0006). It checks only frame sizes; the app checks everything else.

mod launch;
mod relay;

fn main() {
    let result = relay::relay(std::io::stdin().lock(), std::io::stdout().lock(), launch::connect_or_launch);
    if let Err(error) = result {
        eprintln!("scrappy-host: {error}");
        std::process::exit(1);
    }
}
