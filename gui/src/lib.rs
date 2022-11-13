#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub use app::TemplateApp;

#[cfg(test)]
mod test {

    #[test]
    fn test() {
        assert_eq!(1 + 2, 3);
    }

}