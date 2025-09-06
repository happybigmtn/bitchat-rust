use bitcraps::sdk_v2::{config::{Config, Environment}, init};
use bitcraps::sdk_v2::consensus::ConsensusAPI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <base_url> <game_id_hex> <round> [expected_entropy_hash_hex]", args[0]);
        std::process::exit(1);
    }
    let base_url = args[1].clone();
    let game_id_hex = args[2].clone();
    let round: u64 = args[3].parse().unwrap_or(0);
    let expected_hash = args.get(4).cloned();

    let config = Config::builder()
        .api_key("dev")
        .environment(Environment::Local)
        .base_url(&base_url)
        .build()?;
    let ctx = init(config).await?;
    let api = ConsensusAPI::new(ctx);

    if let Some(bundle_json) = api.get_randomness_proof(&game_id_hex, round).await? {
        println!("Proof bundle: {}", bundle_json);
        let vrf_ok = api.verify_vrf_bundle(&bundle_json).await?;
        println!("VRF verify: {}", vrf_ok);
        if let Some(exp) = expected_hash {
            let h = api.hash_randomness_proof(&bundle_json);
            println!("Hash match: {}", h.eq_ignore_ascii_case(&exp));
        }
    } else {
        println!("No randomness proof available yet.");
    }
    Ok(())
}

