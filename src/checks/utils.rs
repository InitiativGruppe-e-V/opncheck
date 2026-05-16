#[macro_export]
macro_rules! skip_check {
    () => {
        return Ok($crate::output::LocalSection::empty())
    };
}
