use tokio_tungstenite::connect_async;
use futures_util::{StreamExt, SinkExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <ws_base_url> <topic> <connections>", args[0]);
        eprintln!("Example: {} ws://127.0.0.1:8080/subscribe game:001122...:events 100", args[0]);
        std::process::exit(1);
    }
    let base = args[1].clone();
    let topic = args[2].clone();
    let conns: usize = args[3].parse()?;

    let mut tasks = Vec::with_capacity(conns);
    for i in 0..conns {
        let url = format!("{}?topic={}", base, urlencoding::encode(&topic));
        tasks.push(tokio::spawn(async move {
            if let Ok((mut ws, _)) = connect_async(url).await {
                // send ping periodically
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            let _ = ws.send(tokio_tungstenite::tungstenite::Message::Ping(vec![])).await;
                        }
                        msg = ws.next() => {
                            if msg.is_none() { break; }
                        }
                    }
                }
            }
        }));
    }
    for t in tasks { let _ = t.await; }
    Ok(())
}

