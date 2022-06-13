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
    let conf = sns_archive::config::weverse::read_config()?;
    let token = sns_archive::config::weverse::read_token(&conf.cookies_file)?;
    sns_archive::weverse::network::download(&conf, &token).await
}
