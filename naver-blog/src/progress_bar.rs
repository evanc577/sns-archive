pub trait ProgressBar {
    fn init(total: usize, description: &str) -> Self;
    fn increment(&self);
    fn destroy(self);
}
