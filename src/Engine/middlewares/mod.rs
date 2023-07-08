pub mod clock;
pub mod window_manager;

pub fn use_common_middlewares(app: &mut super::App)
{
    app.middlewares.try_add(clock::Clock).ok();
}

pub fn use_window_middlewares(app: &mut super::App)
{
    app.middlewares.try_add(window_manager::WindowManager).ok();
}