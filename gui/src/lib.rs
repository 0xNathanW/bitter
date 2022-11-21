#![warn(clippy::all, rust_2018_idioms)]

pub mod widgets;
mod app;
pub use app::App;

#[cfg(test)]
mod test {

    #[test]
    fn test() {
        assert_eq!(1 + 2, 3);
    }

}