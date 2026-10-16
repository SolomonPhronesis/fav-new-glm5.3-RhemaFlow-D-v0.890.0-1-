//! compile-check stub of the tiny part of eframe the app uses
pub mod glow { pub struct Context; }
pub struct CreationContext<'s> { pub egui_ctx: egui::Context, pub _p: std::marker::PhantomData<&'s ()> }
pub struct Frame;
#[derive(Debug)] pub struct Error;
pub trait App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame);
    fn on_exit(&mut self, _gl: Option<&glow::Context>) {}
}
#[derive(Default)] pub struct NativeOptions { pub viewport: egui::ViewportBuilder, pub centered: bool }
pub fn run_native(_n: &str, _o: NativeOptions, _f: Box<dyn FnOnce(&CreationContext<'_>) -> Result<Box<dyn App>, Box<dyn std::error::Error + Send + Sync>>>) -> Result<(), Error> { Ok(()) }
