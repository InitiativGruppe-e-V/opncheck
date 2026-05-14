#[macro_export]
macro_rules! skip_check {
    () => {
        return Ok($crate::plugin::output::LocalSection::empty())
    };
}
