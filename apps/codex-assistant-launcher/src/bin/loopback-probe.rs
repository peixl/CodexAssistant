use codex_assistant_core::launcher::preflight_loopback_reachable;

#[tokio::main]
async fn main() {
    println!("[probe] start");
    match preflight_loopback_reachable().await {
        Ok(()) => println!("[probe] SUCCESS"),
        Err(e) => println!("[probe] FAIL: {e}"),
    }
}
