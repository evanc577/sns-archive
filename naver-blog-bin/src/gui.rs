#![windows_subsystem = "windows"]

use std::sync::atomic::{AtomicUsize, Ordering};

use klask::Settings;
use naver_blog_bin::Args;

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    klask::run_derived::<Args, _>(Settings::default(), |args| {
        if let Err(e) = rt.block_on(async { args.download::<ProgressBar>().await }) {
            eprintln!("Error: {e}");
        }
    })
}

static PB_ID: AtomicUsize = AtomicUsize::new(0);

struct ProgressBar {
    id: usize,
    description: String,
    current: AtomicUsize,
    total: usize,
}

impl naver_blog::ProgressBar for ProgressBar {
    fn init(total: usize, description: &str) -> Self {
        let id = PB_ID.fetch_add(1, Ordering::SeqCst);
        klask::output::progress_bar_with_id(id, description, 0.);
        Self {
            id,
            description: description.to_owned(),
            current: AtomicUsize::new(0),
            total,
        }
    }

    fn increment(&self) {
        let current: usize = self.current.fetch_add(1, Ordering::SeqCst) + 1;
        klask::output::progress_bar_with_id(
            self.id,
            &self.description,
            current as f32 / self.total as f32,
        );
    }

    fn destroy(self) {}
}
