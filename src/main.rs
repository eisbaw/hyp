use clap::Parser;
#[tokio::main]
async fn main() {
    let cli = hyp::cli::Cli::parse();
    let json = cli.json;
    if let Err(err) = hyp::cli::run(cli).await {
        if json {
            eprintln!("{}", serde_json::json!({"error":format!("{err:#}")}));
        } else {
            eprintln!("hyp: {err:#}");
        }
        std::process::exit(if err.to_string().starts_with("conflict:") {
            3
        } else {
            1
        });
    }
}
