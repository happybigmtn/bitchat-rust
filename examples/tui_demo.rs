// Demo of the BitCraps TUI interface
// Run with: cargo run --example tui_demo

use bitcraps::ui::tui::run_tui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting BitCraps Casino TUI Demo...");
    println!("Controls:");
    println!("  Tab - Switch between views");
    println!("  c - Casino view");
    println!("  t - Chat view");
    println!("  p - Peer list");
    println!("  s - Settings");
    println!("  r - Roll dice (in casino)");
    println!("  +/- - Adjust bet amount");
    println!("  q - Quit");
    println!();
    
    // Run the TUI
    run_tui().await?;
    
    println!("Thanks for playing BitCraps!");
    Ok(())
}