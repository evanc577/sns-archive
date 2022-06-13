use std::process;

#[tokio::main]
async fn main() {
    match run().await {
        Ok(_) => process::exit(0),
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    }
}

async fn run() -> Result<(), String> {
    let conf = sns_archive::config::read_config()?;
    let token = sns_archive::config::read_token(&conf.cookies_file)?;
    sns_archive::network::download(&conf, &token).await
}
