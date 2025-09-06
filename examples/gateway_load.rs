use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 6 {
        eprintln!("Usage: {} <base_url> <game_id_hex> <concurrency> <requests_per_client> <player_prefix> [ramp_secs] [duration_secs]", args[0]);
        eprintln!("Example: {} http://127.0.0.1:8080 00112233445566778899aabbccddeeff 100 1000 p 10 60", args[0]);
        std::process::exit(1);
    }
    let base = args[1].clone();
    let game = args[2].clone();
    let conc: usize = args[3].parse()?;
    let per: usize = args[4].parse()?;
    let prefix = args[5].clone();

    let client = reqwest::Client::builder().pool_max_idle_per_host(100).build()?;
    let ramp_secs: u64 = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
    let duration_secs: u64 = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut tasks = Vec::with_capacity(conc);
    let start = std::time::Instant::now();
    for c in 0..conc {
        let client = client.clone();
        let base = base.clone();
        let game = game.clone();
        let player = format!("{}{:08x}", prefix, c);
        tasks.push(tokio::spawn(async move {
            let url = format!("{}/api/v1/games/{}/bets", base, game);
            let start = std::time::Instant::now();
            for i in 0..per {
                let body = serde_json::json!({
                    "player_id_hex": hex::encode(blake3::hash(format!("{}-{}", player, i).as_bytes()).as_bytes()),
                    "bet_type": "pass",
                    "amount": 1u64
                });
                let resp = client.post(&url).json(&body).send().await;
                if resp.is_err() { tokio::time::sleep(Duration::from_millis(5)).await; }
                if duration_secs > 0 && start.elapsed() > Duration::from_secs(duration_secs) { break; }
                if ramp_secs > 0 {
                    let per_client_delay = (ramp_secs * 1000 / (per.max(1) as u64)).max(1);
                    tokio::time::sleep(Duration::from_millis(per_client_delay)).await;
                }
            }
        }));
    }
    for t in tasks { let _ = t.await; }
    let elapsed = start.elapsed();
    println!("Completed {} requests in {:.2}s", conc*per, elapsed.as_secs_f64());
    Ok(())
}
