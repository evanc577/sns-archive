use naver_blog_bin::Args;

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    klask::run_derived::<Args, _>(klask::Settings::default(), |args| {
        if let Err(e) = rt.block_on(async { args.download().await }) {
            eprintln!("Error: {e}");
        }
    })
}
