#![warn(clippy::all, rust_2018_idioms)]

mod widgets;
mod app;
mod gui;
pub use gui::Gui;

#[cfg(test)]
mod test {

    #[test]
    fn test() {
        assert_eq!(1 + 2, 3);
    }

}